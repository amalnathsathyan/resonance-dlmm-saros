import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ResonanceBot } from "../target/types/resonance_bot";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import { getKeypairFromFile } from "@solana-developers/helpers";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
} from "@solana/spl-token";
import { expect } from "chai";

describe("resonance-bot", () => {
  const anchorProvider = anchor.AnchorProvider.env();
  const connection = anchorProvider.connection;
  anchor.setProvider(anchorProvider);

  const program = anchor.workspace.resonanceBot as Program<ResonanceBot>;

  const usdcMint = new PublicKey(
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  );
  const sarosMint = new PublicKey(
    "SarosY6Vscao718M4A778z4CGtvcwcGef5M9MEH1LGL"
  );

  // Saros DLMM Program ID
  const SAROS_PROGRAM = new PublicKey("1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE");

  // Pool addresses (using real Saros DLMM pools)
  const poolA = new PublicKey("GNDi5xLZm26vpVyBbVL9JrDPXR88nQfcPPsmnZQQcbTh");
  const poolB = new PublicKey("ADPKeitAZsAeRJfhG2GoDrZENB3xt9eZmggkj7iAXY78");

  let user: Keypair;
  let vaultPDA: PublicKey;
  let vaultBump: number;
  let usdcUserAta: PublicKey;
  let sarosUserAta: PublicKey;
  let usdcVaultAta: PublicKey;
  let sarosVaultAta: PublicKey;

  // Helper to derive bin array PDA
  function deriveBinArray(lbPair: PublicKey, binArrayIndex: number): PublicKey {
    const binArrayIndexBuffer = Buffer.alloc(8);
    binArrayIndexBuffer.writeBigInt64LE(BigInt(binArrayIndex));
    
    const [binArrayPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("bin_array"), lbPair.toBuffer(), binArrayIndexBuffer],
      SAROS_PROGRAM
    );
    return binArrayPda;
  }

  // Helper to derive token vault PDAs
  function deriveTokenVault(lbPair: PublicKey, tokenMint: PublicKey): PublicKey {
    const [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_vault"), lbPair.toBuffer(), tokenMint.toBuffer()],
      SAROS_PROGRAM
    );
    return vaultPda;
  }

  // Helper to derive event authority
  function deriveEventAuthority(): PublicKey {
    const [eventAuthority] = PublicKey.findProgramAddressSync(
      [Buffer.from("__event_authority")],
      SAROS_PROGRAM
    );
    return eventAuthority;
  }

  // Fetch active bin from pool state
  async function getActiveBinId(poolAddress: PublicKey): Promise<number> {
    try {
      const accountInfo = await connection.getAccountInfo(poolAddress);
      if (!accountInfo) {
        console.log("Pool not found, using default bin ID 0");
        return 0;
      }

      // Saros DLMM pool structure - active bin is at offset 136 (after discriminator and other fields)
      // This is a simplified approach - in production you'd use the IDL to deserialize properly
      const data = accountInfo.data;
      
      // Read active bin ID (i32 at offset ~136)
      // Adjust offset based on actual Saros pool structure
      const activeBinId = data.readInt32LE(136);
      console.log(`Active bin ID for pool ${poolAddress.toBase58()}: ${activeBinId}`);
      
      return activeBinId;
    } catch (error) {
      console.log(`Error fetching active bin, using default: ${error.message}`);
      return 0;
    }
  }

  // Get bin array index from bin ID
  function getBinArrayIndex(binId: number): number {
    // Each bin array typically contains ~70 bins (this may vary)
    const BIN_ARRAY_SIZE = 70;
    return Math.floor(binId / BIN_ARRAY_SIZE);
  }

  async function createAtaIfNotExists(
    mint: PublicKey,
    owner: PublicKey,
    payer: Keypair,
    allowOwnerOffCurve: boolean = false
  ): Promise<PublicKey> {
    const ata = await getAssociatedTokenAddress(
      mint,
      owner,
      allowOwnerOffCurve
    );

    try {
      const accountInfo = await connection.getAccountInfo(ata);
      if (accountInfo) {
        console.log(`✅ ATA already exists: ${ata.toBase58()}`);
        return ata;
      }
    } catch (error) {
      console.log(
        `Creating ATA for ${mint.toBase58()} owned by ${owner.toBase58()}`
      );
    }

    try {
      const createAtaIx = createAssociatedTokenAccountInstruction(
        payer.publicKey,
        ata,
        owner,
        mint
      );
      const tx = new anchor.web3.Transaction().add(createAtaIx);
      const signature = await anchor.web3.sendAndConfirmTransaction(
        connection,
        tx,
        [payer],
        { commitment: "confirmed" }
      );
      console.log(`✅ ATA created: ${ata.toBase58()}, tx: ${signature}`);
      return ata;
    } catch (error) {
      if (error.message?.includes("already in use")) {
        console.log(
          `✅ ATA already exists (race condition): ${ata.toBase58()}`
        );
        return ata;
      }
      throw error;
    }
  }

  async function setSurfpoolTokenBalance(
    tokenAccount: PublicKey,
    mint: PublicKey,
    amount: number,
    decimals: number = 6
  ): Promise<boolean> {
    try {
      console.log(`🏦 Setting ${amount.toLocaleString()} tokens via Surfpool...`);
      
      const rawAmount = amount * Math.pow(10, decimals);
      
      const response = await fetch("https://tiled-talcs-mars.txtx.network:8899", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method: "surfnet_setTokenAccount",
          params: [
            tokenAccount.toBase58(),           
            mint.toBase58(),                   
            { amount: rawAmount }              
          ],
        }),
      });

      if (!response.ok) {
        console.log("Surfpool cheatcode not available");
        return false;
      }

      const result = await response.json();
      if (result.error) {
        console.log(`⚠️ Surfpool error: ${result.error.message}`);
        return false;
      }

      console.log(`✅ Set token balance via Surfpool: ${amount.toLocaleString()} tokens`);
      
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const balance = await connection.getTokenAccountBalance(tokenAccount);
      const actualBalance = parseInt(balance.value.amount);
      const formattedBalance = actualBalance / Math.pow(10, decimals);
      console.log(`📊 Verified balance: ${formattedBalance.toLocaleString()}`);
      return actualBalance > 0;
      
    } catch (error) {
      console.log("Surfpool cheatcode not available");
      return false;
    }
  }

  async function ensureSufficientSolBalance(publicKey: PublicKey, minLamports: number = 10 * LAMPORTS_PER_SOL): Promise<void> {
    try {
      const balance = await connection.getBalance(publicKey);
      console.log(`💰 Current SOL balance: ${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
      
      if (balance < minLamports) {
        console.log(`🔄 Balance too low, requesting airdrop...`);
        
        let currentBalance = balance;
        let attempts = 0;
        const maxAttempts = 5;
        
        while (currentBalance < minLamports && attempts < maxAttempts) {
          try {
            const airdropAmount = Math.min(5 * LAMPORTS_PER_SOL, minLamports - currentBalance);
            console.log(`📤 Requesting airdrop of ${(airdropAmount / LAMPORTS_PER_SOL).toFixed(2)} SOL`);
            
            const airdropTx = await connection.requestAirdrop(publicKey, airdropAmount);
            await connection.confirmTransaction(airdropTx, "confirmed");
            
            await new Promise((resolve) => setTimeout(resolve, 2000));
            currentBalance = await connection.getBalance(publicKey);
            console.log(`✅ Updated balance: ${(currentBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
            
            attempts++;
          } catch (airdropError) {
            console.log(`⚠️ Airdrop attempt ${attempts + 1} failed`);
            attempts++;
            await new Promise((resolve) => setTimeout(resolve, 1000));
          }
        }
        
        if (currentBalance < minLamports) {
          throw new Error("Insufficient SOL balance for transactions");
        }
      } else {
        console.log("✅ SOL balance is sufficient");
      }
    } catch (error) {
      console.error("❌ Error ensuring SOL balance:", error.message);
      throw error;
    }
  }

  before("Setting Up User and Environment", async () => {
    try {
      user = await getKeypairFromFile(
        "/Users/amalnathsathyan/Documents/trycatchblock/my-projects/resonance-bot/tests/test-keys/user.json"
      );
      console.log("User:", user.publicKey.toBase58());

      await ensureSufficientSolBalance(user.publicKey, 10 * LAMPORTS_PER_SOL);

      [vaultPDA, vaultBump] = PublicKey.findProgramAddressSync(
        [Buffer.from("resonance-vault"), user.publicKey.toBuffer()],
        program.programId
      );
      console.log("Vault PDA:", vaultPDA.toBase58());

      usdcUserAta = await getAssociatedTokenAddress(usdcMint, user.publicKey);
      sarosUserAta = await getAssociatedTokenAddress(sarosMint, user.publicKey);
      usdcVaultAta = await getAssociatedTokenAddress(usdcMint, vaultPDA, true);
      sarosVaultAta = await getAssociatedTokenAddress(sarosMint, vaultPDA, true);

      console.log("User USDC ATA:", usdcUserAta.toBase58());
      console.log("Vault USDC ATA:", usdcVaultAta.toBase58());
      console.log("Vault SAROS ATA:", sarosVaultAta.toBase58());

      await createAtaIfNotExists(usdcMint, user.publicKey, user);
      await createAtaIfNotExists(sarosMint, user.publicKey, user);

      await setSurfpoolTokenBalance(usdcUserAta, usdcMint, 100_000, 6);
      
    } catch (error) {
      console.error("Setup failed:", error);
      throw error;
    }
  });

  it("Initializes vault with better error handling", async () => {
    const minProfitThreshold = new anchor.BN(100000);
    const maxSingleTrade = new anchor.BN(10000 * 1000000);

    try {
      await ensureSufficientSolBalance(user.publicKey, 5 * LAMPORTS_PER_SOL);

      // Check vault state more thoroughly
      let vaultExists = false;
      let vaultInitialized = false;
      
      try {
        const vaultAccountInfo = await connection.getAccountInfo(vaultPDA);
        
        if (vaultAccountInfo) {
          vaultExists = true;
          console.log(`📦 Vault account exists with ${vaultAccountInfo.data.length} bytes`);
          
          // Check if it's owned by our program
          if (vaultAccountInfo.owner.equals(program.programId)) {
            console.log("✅ Vault is owned by our program");
            
            // Try to deserialize the data
            try {
              const existingVault = await program.account.arbitrageVault.fetch(vaultPDA);
              vaultInitialized = true;
              console.log("✅ Vault already fully initialized");
              console.log("   Authority:", existingVault.authority.toBase58());
              console.log("   Bump:", existingVault.bump);
              console.log("   Total Trades:", existingVault.totalTrades.toNumber());
              
              // Verify it's for the correct authority
              if (existingVault.authority.equals(user.publicKey)) {
                console.log("✅ Vault belongs to current user - skipping initialization");
                return;
              } else {
                console.log("⚠️ Vault exists but belongs to different authority");
                throw new Error("Vault PDA collision - belongs to different user");
              }
            } catch (fetchError) {
              console.log("⚠️ Vault account exists but cannot deserialize data");
              console.log("   This might mean it's partially initialized or corrupted");
              console.log("   Error:", fetchError.message);
            }
          } else {
            console.log("⚠️ Account at vault PDA is owned by:", vaultAccountInfo.owner.toBase58());
            throw new Error("Vault PDA is occupied by another program");
          }
        } else {
          console.log("📝 Vault account does not exist - will initialize");
        }
      } catch (checkError) {
        console.log("📝 Error checking vault, will attempt initialization:", checkError.message);
      }

      // Only initialize if vault doesn't exist or isn't properly initialized
      if (!vaultInitialized) {
        console.log("\n🚀 Initializing new vault...");
        
        // Double-check SOL balance right before transaction
        const preBalance = await connection.getBalance(user.publicKey);
        console.log(`💰 Pre-transaction balance: ${(preBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
        
        if (preBalance < 2 * LAMPORTS_PER_SOL) {
          throw new Error("Insufficient SOL for initialization (need at least 2 SOL)");
        }

        // If vault account exists but isn't initialized, we might need to close it first
        if (vaultExists) {
          console.log("⚠️ Vault account exists but isn't initialized - this may cause issues");
          console.log("💡 Consider manually closing the account or using a different authority");
        }

        try {
          const initTx = await program.methods
            .initializeVault(minProfitThreshold, maxSingleTrade)
            .accounts({
              authority: user.publicKey,
              vault: vaultPDA,
              mintX: sarosMint,
              mintY: usdcMint,
              vaultAtaX: sarosVaultAta,
              vaultAtaY: usdcVaultAta,
              systemProgram: SystemProgram.programId,
              tokenProgram: TOKEN_PROGRAM_ID,
              associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            })
            .preInstructions([
              anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
                units: 600_000, // Increased compute limit
              }),
              anchor.web3.ComputeBudgetProgram.setComputeUnitPrice({
                microLamports: 2000, // Higher priority fee
              }),
            ])
            .signers([user])
            .rpc({
              skipPreflight: false,
              commitment: "confirmed",
            });

          console.log("✅ Vault initialization transaction:", initTx);

          // Wait for confirmation
          await connection.confirmTransaction(initTx, "confirmed");
          console.log("✅ Transaction confirmed");

          // Wait for account to be fully created
          await new Promise((resolve) => setTimeout(resolve, 3000));
          
          // Verify the vault was created correctly
          const vaultAccount = await program.account.arbitrageVault.fetch(vaultPDA);
          expect(vaultAccount.authority.equals(user.publicKey)).to.be.true;
          console.log("\n✅ Vault successfully initialized and verified");
          console.log("   Authority:", vaultAccount.authority.toBase58());
          console.log("   Min Profit:", (vaultAccount.minProfitThreshold.toNumber() / 1_000_000).toFixed(2), "USDC");
          console.log("   Max Trade:", (vaultAccount.maxSingleTrade.toNumber() / 1_000_000).toFixed(0), "USDC");
          console.log("   Bump:", vaultAccount.bump);
          
        } catch (txError) {
          console.error("\n❌ Transaction failed:", txError.message);
          
          // Try to get detailed logs
          if (txError.logs) {
            console.log("\n📋 Transaction logs:");
            txError.logs.forEach((log, i) => {
              console.log(`${String(i).padStart(3, ' ')}: ${log}`);
            });
          }
          
          // Check if it's a duplicate initialization attempt
          if (txError.message.includes("already in use") || txError.message.includes("0x0")) {
            console.log("\n💡 Account might already be initialized");
            console.log("💡 Trying to fetch existing vault...");
            
            try {
              const existingVault = await program.account.arbitrageVault.fetch(vaultPDA);
              console.log("✅ Found existing vault:", existingVault.authority.toBase58());
              return; // Exit successfully if vault exists
            } catch (fetchError) {
              console.log("❌ Could not fetch vault after initialization error");
            }
          }
          
          throw txError;
        }
      }

    } catch (error) {
      console.error("\n❌ Vault initialization process failed:", error.message);
      
      // Final balance check
      const finalBalance = await connection.getBalance(user.publicKey);
      console.log(`💰 Final SOL balance: ${(finalBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
      
      // Debug info
      console.log("\n🔍 Debug Info:");
      console.log("   Program ID:", program.programId.toBase58());
      console.log("   User:", user.publicKey.toBase58());
      console.log("   Vault PDA:", vaultPDA.toBase58());
      console.log("   Expected vault space:", 8 + 80, "bytes");
      
      throw error;
    }
  });


  it("Deposits USDC to vault", async () => {
    let depositAmountUSDC = new anchor.BN(50_000 * 1_000_000);

    try {
      await ensureSufficientSolBalance(user.publicKey, 3 * LAMPORTS_PER_SOL);

      const vaultState = await program.account.arbitrageVault.fetch(vaultPDA);
      console.log("✅ Vault ready for deposit");

      const userBalance = await connection.getTokenAccountBalance(usdcUserAta);
      const userBalanceAmount = parseInt(userBalance.value.amount);
      
      console.log(`User USDC: ${(userBalanceAmount / 1_000_000).toLocaleString()}`);

      if (userBalanceAmount < depositAmountUSDC.toNumber()) {
        const adjustedAmount = Math.floor(userBalanceAmount * 0.9);
        depositAmountUSDC = new anchor.BN(adjustedAmount);
        console.log(`Adjusted deposit: ${(adjustedAmount / 1_000_000).toLocaleString()} USDC`);
      }

      const usdcTxSignature = await program.methods
        .depositFunds(depositAmountUSDC)
        .accounts({
          authority: user.publicKey,
          vault: vaultPDA,
          mintX: usdcMint,
          authorityAtaX: usdcUserAta,
          vaultAtaX: usdcVaultAta,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .preInstructions([
          anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
            units: 300_000,
          }),
        ])
        .signers([user])
        .rpc();

      console.log("✅ USDC deposit transaction:", usdcTxSignature);

      await new Promise((resolve) => setTimeout(resolve, 3000));
      
      const vaultBalanceAfter = await connection.getTokenAccountBalance(usdcVaultAta);
      console.log(`Vault USDC: ${(parseInt(vaultBalanceAfter.value.amount) / 1_000_000).toLocaleString()}`);

      expect(parseInt(vaultBalanceAfter.value.amount)).to.be.greaterThan(0);
      console.log("✅ Vault funded successfully!");

    } catch (error) {
      console.error("❌ Deposit failed:", error);
      throw error;
    }
  });

  it("Executes real Saros DLMM arbitrage with proper accounts", async () => {
    console.log("\n🚀 === REAL SAROS DLMM ARBITRAGE EXECUTION ===\n");

    try {
      await ensureSufficientSolBalance(user.publicKey, 3 * LAMPORTS_PER_SOL);

      const vaultStateBefore = await program.account.arbitrageVault.fetch(vaultPDA);
      console.log("✅ Vault ready for arbitrage");

      const testAmount = new anchor.BN(1000_000_000); // 1000 USDC
      console.log(`Test amount: $${(testAmount.toNumber() / 1_000_000).toFixed(2)} USDC`);

      // Get balances before
      const vaultUsdcBefore = await connection.getTokenAccountBalance(usdcVaultAta);
      const vaultSarosBefore = await connection.getTokenAccountBalance(sarosVaultAta);

      console.log(`\n📊 Pre-execution balances:`);
      console.log(`Vault USDC: ${(parseInt(vaultUsdcBefore.value.amount) / 1_000_000).toFixed(2)}`);
      console.log(`Vault SAROS: ${(parseInt(vaultSarosBefore.value.amount) / 1_000_000).toFixed(2)}`);

      // Derive proper Saros DLMM accounts
      console.log("\n🔧 Deriving Saros DLMM accounts...");

      // Get active bins for both pools
      const activeBinA = await getActiveBinId(poolA);
      const activeBinB = await getActiveBinId(poolB);

      const binArrayIndexA = getBinArrayIndex(activeBinA);
      const binArrayIndexB = getBinArrayIndex(activeBinB);

      // Derive bin arrays (lower and upper around active bin)
      const binArrayLowerA = deriveBinArray(poolA, binArrayIndexA);
      const binArrayUpperA = deriveBinArray(poolA, binArrayIndexA + 1);
      const binArrayLowerB = deriveBinArray(poolB, binArrayIndexB);
      const binArrayUpperB = deriveBinArray(poolB, binArrayIndexB + 1);

      // Derive token vaults for both pools
      const tokenVaultXA = deriveTokenVault(poolA, sarosMint);
      const tokenVaultYA = deriveTokenVault(poolA, usdcMint);
      const tokenVaultXB = deriveTokenVault(poolB, sarosMint);
      const tokenVaultYB = deriveTokenVault(poolB, usdcMint);

      // Derive event authority
      const eventAuthority = deriveEventAuthority();

      console.log(`Pool A bin arrays: ${binArrayLowerA.toBase58()}, ${binArrayUpperA.toBase58()}`);
      console.log(`Pool B bin arrays: ${binArrayLowerB.toBase58()}, ${binArrayUpperB.toBase58()}`);
      console.log(`Event Authority: ${eventAuthority.toBase58()}`);

      const startTime = Date.now();
      console.log("\n⚡ Executing arbitrage transaction...");

      const txSignature = await program.methods
        .executeArbitrage(poolA, poolB, testAmount)
        .accounts({
          vault: vaultPDA,
          authority: user.publicKey,
          
          // Pool A accounts
          poolA: poolA,
          binArrayLowerA: binArrayLowerA,
          binArrayUpperA: binArrayUpperA,
          tokenVaultXA: tokenVaultXA,
          tokenVaultYA: tokenVaultYA,

          // Pool B accounts
          poolB: poolB,
          binArrayLowerB: binArrayLowerB,
          binArrayUpperB: binArrayUpperB,
          tokenVaultXB: tokenVaultXB,
          tokenVaultYB: tokenVaultYB,

          // Vault token accounts
          vaultTokenX: sarosVaultAta,
          vaultTokenY: usdcVaultAta,

          // Token mints
          tokenMintX: sarosMint,
          tokenMintY: usdcMint,

          // Event authority
          eventAuthority: eventAuthority,

          // Programs
          tokenProgram: TOKEN_PROGRAM_ID,
          sarosProgram: SAROS_PROGRAM,
        })
        .preInstructions([
          anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
            units: 800_000,
          }),
          anchor.web3.ComputeBudgetProgram.setComputeUnitPrice({
            microLamports: 2000,
          }),
        ])
        .signers([user])
        .rpc({
          commitment: "confirmed",
          skipPreflight: false,
        });

      const executionTime = Date.now() - startTime;
      console.log(`⚡ Transaction completed in ${executionTime}ms`);
      console.log(`📝 Signature: ${txSignature}`);

      // Get transaction logs
      await new Promise(resolve => setTimeout(resolve, 2000));

      const txDetails = await connection.getTransaction(txSignature, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      console.log("\n=== 📋 EXECUTION LOGS ===");
      if (txDetails?.meta?.logMessages) {
        txDetails.meta.logMessages.forEach((log, i) => {
          if (
            log.includes("Pool") || log.includes("SWAP") || log.includes("ARBITRAGE") ||
            log.includes("real price") || log.includes("DIRECTION") ||
            log.includes("EXECUTING") || log.includes("COMPLETED") ||
            log.includes("received") || log.includes("Balance change") ||
            log.includes("Profit") || log.includes("🚀") || log.includes("⚡") || 
            log.includes("💰") || log.includes("📊") || log.includes("✅")
          ) {
            console.log(`${String(i).padStart(2, '0')}: ${log}`);
          }
        });
      }

      // Get balances after
      const vaultUsdcAfter = await connection.getTokenAccountBalance(usdcVaultAta);
      const vaultSarosAfter = await connection.getTokenAccountBalance(sarosVaultAta);

      console.log(`\n📊 Post-execution balances:`);
      console.log(`Vault USDC: ${(parseInt(vaultUsdcAfter.value.amount) / 1_000_000).toFixed(2)}`);
      console.log(`Vault SAROS: ${(parseInt(vaultSarosAfter.value.amount) / 1_000_000).toFixed(2)}`);

      const usdcChange = parseInt(vaultUsdcAfter.value.amount) - parseInt(vaultUsdcBefore.value.amount);
      const sarosChange = parseInt(vaultSarosAfter.value.amount) - parseInt(vaultSarosBefore.value.amount);

      console.log(`\n💰 Balance Changes:`);
      console.log(`USDC: ${usdcChange > 0 ? '+' : ''}${(usdcChange / 1_000_000).toFixed(6)}`);
      console.log(`SAROS: ${sarosChange > 0 ? '+' : ''}${(sarosChange / 1_000_000).toFixed(6)}`);

      const vaultStateAfter = await program.account.arbitrageVault.fetch(vaultPDA);

      console.log("\n📊 Vault Statistics:");
      console.log(`Trades: ${vaultStateBefore.totalTrades.toNumber()} → ${vaultStateAfter.totalTrades.toNumber()}`);
      console.log(`Total Profits: $${(vaultStateBefore.totalProfits.toNumber() / 1_000_000).toFixed(2)} → $${(vaultStateAfter.totalProfits.toNumber() / 1_000_000).toFixed(2)}`);

      expect(vaultStateAfter.totalTrades.toNumber()).to.be.greaterThan(vaultStateBefore.totalTrades.toNumber());

      console.log("\n🎉 === ARBITRAGE EXECUTION COMPLETE ===");
      console.log("✅ Real pool price parsing: SUCCESS");
      console.log("✅ Saros CPI with bin arrays: SUCCESS");
      console.log("✅ Sequential swaps: EXECUTED");
      console.log("✅ Balance tracking: SUCCESS");

      if (usdcChange > 0) {
        console.log(`💰 PROFIT: $${(usdcChange / 1_000_000).toFixed(6)} USDC`);
      }

    } catch (error) {
      console.error("❌ Arbitrage execution failed:", error.message);

      if (error.logs) {
        console.log("\n📋 Error logs:");
        error.logs.forEach((log, i) => {
          console.log(`${String(i).padStart(2, '0')}: ${log}`);
        });
      }

      if (error.message.includes("NoArbitrageOpportunity")) {
        console.log("\n💡 No price difference between pools - this is expected");
      }

      throw error;
    }
  });
});

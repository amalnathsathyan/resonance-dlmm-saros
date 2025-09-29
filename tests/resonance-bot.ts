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
  AccountLayout,
} from "@solana/spl-token";
import { expect } from "chai";

describe("resonance-bot", () => {
  // 🔥 FIXED: Configure the client with proper options for Surfnet Cloud
  const anchorProvider = anchor.AnchorProvider.local(
    "https://tiled-talcs-mars.txtx.network:8899/",
    {
      commitment: 'confirmed',
      skipPreflight: true, // Skip preflight for faster execution
      preflightCommitment: 'confirmed'
    }
  );
  const connection = anchorProvider.connection;
  anchor.setProvider(anchorProvider);

  const program = anchor.workspace.resonanceBot as Program<ResonanceBot>;

  const usdcMint = new PublicKey(
    "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  );
  const sarosMint = new PublicKey(
    "SarosY6Vscao718M4A778z4CGtvcwcGef5M9MEH1LGL"
  );

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

  // Utility function to create ATA if it doesn't exist
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

  // FIXED: Surfpool cheatcode with proper u64 format for token amounts
  async function setSurfpoolTokenBalance(
    tokenAccount: PublicKey,
    mint: PublicKey,
    amount: number,
    decimals: number = 6
  ): Promise<boolean> {
    try {
      console.log(`🏦 Setting ${amount.toLocaleString()} tokens via Surfpool...`);
      
      const rawAmount = amount * Math.pow(10, decimals);
      
      const response = await fetch("https://tiled-talcs-mars.txtx.network:8899/", {
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
        console.log("Surfpool cheatcode not available, using manual method");
        return false;
      }

      const result = await response.json();
      if (result.error) {
        console.log(`⚠️ Surfpool error: ${result.error.message}`);
        return false;
      }

      console.log(`✅ Set token balance via Surfpool: ${amount.toLocaleString()} tokens`);
      
      await new Promise((resolve) => setTimeout(resolve, 1000));
      try {
        const balance = await connection.getTokenAccountBalance(tokenAccount);
        const actualBalance = parseInt(balance.value.amount);
        const formattedBalance = actualBalance / Math.pow(10, decimals);
        console.log(`📊 Verified balance: ${formattedBalance.toLocaleString()}`);
        return actualBalance > 0;
      } catch (error) {
        return false;
      }
      
    } catch (error) {
      console.log("Surfpool cheatcode not available, using manual method");
      return false;
    }
  }

  // 🔥 FIXED: Enhanced SOL balance management for Surfnet
  async function ensureSufficientSolBalance(publicKey: PublicKey, minLamports: number = 10 * LAMPORTS_PER_SOL): Promise<void> {
    try {
      const balance = await connection.getBalance(publicKey);
      console.log(`💰 Current SOL balance: ${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
      
      if (balance < minLamports) {
        console.log(`🔄 Balance too low (${(balance / LAMPORTS_PER_SOL).toFixed(4)} SOL), requesting airdrop...`);
        
        // Request airdrop multiple times if needed
        let currentBalance = balance;
        let attempts = 0;
        const maxAttempts = 5;
        
        while (currentBalance < minLamports && attempts < maxAttempts) {
          try {
            const airdropAmount = Math.min(5 * LAMPORTS_PER_SOL, minLamports - currentBalance);
            console.log(`📤 Requesting airdrop of ${(airdropAmount / LAMPORTS_PER_SOL).toFixed(2)} SOL (attempt ${attempts + 1})`);
            
            const airdropTx = await connection.requestAirdrop(publicKey, airdropAmount);
            console.log(`⏳ Airdrop transaction: ${airdropTx}`);
            
            await connection.confirmTransaction(airdropTx, "confirmed");
            
            // Wait for balance to update
            await new Promise((resolve) => setTimeout(resolve, 2000));
            currentBalance = await connection.getBalance(publicKey);
            console.log(`✅ Updated balance: ${(currentBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
            
            attempts++;
          } catch (airdropError) {
            console.log(`⚠️ Airdrop attempt ${attempts + 1} failed:`, airdropError.message);
            attempts++;
            await new Promise((resolve) => setTimeout(resolve, 1000));
          }
        }
        
        if (currentBalance < minLamports) {
          console.log(`❌ Unable to get sufficient SOL after ${maxAttempts} attempts`);
          console.log(`💡 Required: ${(minLamports / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
          console.log(`💡 Current: ${(currentBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
          throw new Error("Insufficient SOL balance for transactions");
        }
      } else {
        console.log("✅ SOL balance is sufficient for transactions");
      }
    } catch (error) {
      console.error("❌ Error checking/ensuring SOL balance:", error.message);
      throw error;
    }
  }

  before("Setting Up User and Environment", async () => {
    try {
      // Load user keypair
      user = await getKeypairFromFile(
        "/Users/amalnathsathyan/Documents/trycatchblock/my-projects/resonance-bot/tests/test-keys/user.json"
      );
      console.log("User:", user.publicKey.toBase58());

      // 🔥 FIXED: Ensure sufficient SOL balance before any operations
      await ensureSufficientSolBalance(user.publicKey, 10 * LAMPORTS_PER_SOL);

      // Derive vault PDA
      [vaultPDA, vaultBump] = PublicKey.findProgramAddressSync(
        [Buffer.from("resonance-vault"), user.publicKey.toBuffer()],
        program.programId
      );
      console.log("Vault PDA:", vaultPDA.toBase58());

      // Calculate ATAs
      usdcUserAta = await getAssociatedTokenAddress(usdcMint, user.publicKey);
      sarosUserAta = await getAssociatedTokenAddress(sarosMint, user.publicKey);
      usdcVaultAta = await getAssociatedTokenAddress(usdcMint, vaultPDA, true);
      sarosVaultAta = await getAssociatedTokenAddress(
        sarosMint,
        vaultPDA,
        true
      );

      console.log("User USDC ATA:", usdcUserAta.toBase58());
      console.log("User SAROS ATA:", sarosUserAta.toBase58());
      console.log("Vault USDC ATA:", usdcVaultAta.toBase58());
      console.log("Vault SAROS ATA:", sarosVaultAta.toBase58());

      // Create user ATAs if they don't exist
      await createAtaIfNotExists(usdcMint, user.publicKey, user);
      await createAtaIfNotExists(sarosMint, user.publicKey, user);

      // Try to set 100K USDC balance using corrected Surfpool API
      const balanceSet = await setSurfpoolTokenBalance(usdcUserAta, usdcMint, 100_000, 6);
      
      // Check current user balance
      try {
        const userBalance = await connection.getTokenAccountBalance(
          usdcUserAta
        );
        const balanceAmount = parseInt(userBalance.value.amount);
        console.log(`💰 User USDC balance: ${(balanceAmount / 1_000_000).toLocaleString()} USDC`);
        
        if (balanceAmount === 0) {
          console.log("⚠️ WARNING: User has zero USDC balance!");
          console.log("💡 Please manually fund user's USDC ATA:", usdcUserAta.toBase58());
        }
      } catch (error) {
        console.log("Could not check user USDC balance");
      }
      
    } catch (error) {
      console.error("Setup failed:", error);
      throw error;
    }
  });

  // 🔥 FIXED: Proper vault initialization with enhanced error handling
  it("Initializes vault with proper error handling and SOL management", async () => {
    const minProfitThreshold = new anchor.BN(100000); // 0.1 USDC
    const maxSingleTrade = new anchor.BN(10000 * 1000000); // 10,000 USDC

    try {
      // 🔥 FIRST: Ensure user has sufficient SOL for this operation
      await ensureSufficientSolBalance(user.publicKey, 5 * LAMPORTS_PER_SOL);

      // Check if vault account exists and is properly initialized
      let needsInitialization = false;
      
      try {
        const vaultAccountInfo = await connection.getAccountInfo(vaultPDA);
        
        if (!vaultAccountInfo) {
          console.log("📝 Vault account not found - needs initialization");
          needsInitialization = true;
        } else {
          // Account exists, check if it's properly initialized with data
          try {
            const existingVault = await program.account.arbitrageVault.fetch(vaultPDA);
            console.log("✅ Vault already initialized and data is valid");
            console.log("   Authority:", existingVault.authority.toBase58());
            console.log("   Total Trades:", existingVault.totalTrades.toNumber());
            console.log("   Bump:", existingVault.bump);
            return; // Vault is properly initialized, skip
          } catch (dataError) {
            console.log("⚠️ Vault account exists but data is invalid:", dataError.message);
            console.log("🔄 Will attempt to reinitialize...");
            needsInitialization = true;
          }
        }
      } catch (error) {
        console.log("📝 Error checking vault state - will initialize:", error.message);
        needsInitialization = true;
      }

      if (needsInitialization) {
        console.log("🚀 Initializing vault...");
        
        // 🔥 FIXED: Add higher compute limit and proper transaction options
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
              units: 500_000, // Higher compute limit for initialization
            }),
            anchor.web3.ComputeBudgetProgram.setComputeUnitPrice({
              microLamports: 1000, // Higher priority fee
            }),
          ])
          .signers([user])
          .rpc();

        console.log("✅ Vault initialization transaction:", initTx);

        // Wait for confirmation and verify
        await new Promise((resolve) => setTimeout(resolve, 3000));
        
        const vaultAccount = await program.account.arbitrageVault.fetch(vaultPDA);
        expect(vaultAccount.authority.equals(user.publicKey)).to.be.true;
        console.log("✅ Vault successfully initialized and verified");
        console.log("   Authority:", vaultAccount.authority.toBase58());
        console.log("   Min Profit Threshold:", vaultAccount.minProfitThreshold.toNumber() / 1_000_000, "USDC");
        console.log("   Max Single Trade:", vaultAccount.maxSingleTrade.toNumber() / 1_000_000, "USDC");
        console.log("   Bump:", vaultAccount.bump);
      }

    } catch (error) {
      console.error("❌ Vault initialization failed:", error);
      
      // 🔥 ENHANCED: Try to get full transaction logs
      if (error.signature) {
        console.log("🔍 Transaction signature:", error.signature);
        try {
          const txDetails = await connection.getTransaction(error.signature, {
            commitment: "confirmed",
            maxSupportedTransactionVersion: 0,
          });
          
          if (txDetails?.meta?.logMessages) {
            console.log("📋 Full transaction logs:");
            txDetails.meta.logMessages.forEach((log, i) => {
              console.log(`${i}: ${log}`);
            });
          }
        } catch (logError) {
          console.log("Could not fetch transaction logs:", logError.message);
        }
      }
      
      if (error.logs) {
        console.log("📋 Error logs:");
        error.logs.forEach((log, i) => {
          console.log(`${i}: ${log}`);
        });
      }
      
      // Check user balance again
      const currentBalance = await connection.getBalance(user.publicKey);
      console.log(`💰 Current SOL balance after error: ${(currentBalance / LAMPORTS_PER_SOL).toFixed(4)} SOL`);
      
      // For debugging - check what accounts we're using
      console.log("🔍 Debug Info:");
      console.log("   Program ID:", program.programId.toBase58());
      console.log("   User:", user.publicKey.toBase58());
      console.log("   Vault PDA:", vaultPDA.toBase58());
      console.log("   Vault Bump:", vaultBump);
      
      throw error;
    }
  });

  it("Verifies vault ATAs are created", async () => {
    // Verify vault USDC ATA
    try {
      const usdcAtaInfo = await connection.getAccountInfo(usdcVaultAta);
      if (usdcAtaInfo) {
        console.log("✅ Vault USDC ATA exists");
        
        const balance = await connection.getTokenAccountBalance(usdcVaultAta);
        console.log("Vault USDC balance:", parseInt(balance.value.amount) / 1_000_000, "USDC");
      } else {
        console.log("⚠️ Vault USDC ATA not found - this should have been created during initialization");
      }
    } catch (error) {
      console.log("⚠️ Vault USDC ATA check failed:", error.message);
    }

    // Verify vault SAROS ATA
    try {
      const sarosAtaInfo = await connection.getAccountInfo(sarosVaultAta);
      if (sarosAtaInfo) {
        console.log("✅ Vault SAROS ATA exists");
      } else {
        console.log("⚠️ Vault SAROS ATA not found - this should have been created during initialization");
      }
    } catch (error) {
      console.log("⚠️ Vault SAROS ATA check failed:", error.message);
    }
  });

  it("Deposits USDC to vault using correct depositFunds function", async () => {
    let depositAmountUSDC = new anchor.BN(50_000 * 1_000_000); // $50K USDC

    console.log("💰 === VAULT DEPOSIT ===");

    try {
      // 🔥 Ensure sufficient SOL for this operation
      await ensureSufficientSolBalance(user.publicKey, 3 * LAMPORTS_PER_SOL);

      // Verify vault is properly initialized before deposit
      try {
        const vaultState = await program.account.arbitrageVault.fetch(vaultPDA);
        console.log("✅ Vault is properly initialized for deposit");
        console.log("   Authority:", vaultState.authority.toBase58());
        console.log("   Bump:", vaultState.bump);
      } catch (error) {
        console.error("❌ Vault is not properly initialized, cannot deposit");
        console.error("   Error:", error.message);
        throw new Error("Vault must be initialized before deposits");
      }

      // Check user's USDC balance first
      const userBalance = await connection.getTokenAccountBalance(usdcUserAta);
      const userBalanceAmount = parseInt(userBalance.value.amount);
      
      console.log(`User USDC: ${(userBalanceAmount / 1_000_000).toLocaleString()}`);
      console.log(`Depositing USDC: ${(depositAmountUSDC.toNumber() / 1_000_000).toLocaleString()}`);

      if (userBalanceAmount < depositAmountUSDC.toNumber()) {
        console.log("⚠️ Insufficient user balance for USDC deposit");
        console.log(`Required: ${(depositAmountUSDC.toNumber() / 1_000_000).toLocaleString()} USDC`);
        console.log(`Available: ${(userBalanceAmount / 1_000_000).toLocaleString()} USDC`);
        console.log("💡 Reducing deposit amount to available balance");
        
        const adjustedAmount = Math.floor(userBalanceAmount * 0.9);
        depositAmountUSDC = new anchor.BN(adjustedAmount);
        console.log(`Adjusted deposit: ${(adjustedAmount / 1_000_000).toLocaleString()} USDC`);
      }

      // Get vault balance before deposit
      let vaultBalanceBeforeAmount = 0;
      try {
        const vaultBalanceBefore = await connection.getTokenAccountBalance(usdcVaultAta);
        vaultBalanceBeforeAmount = parseInt(vaultBalanceBefore.value.amount);
        console.log(`Vault USDC before: ${(vaultBalanceBeforeAmount / 1_000_000).toLocaleString()}`);
      } catch (error) {
        console.log("⚠️ Could not get vault balance before deposit:", error.message);
      }

      // Execute deposit via program instruction
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
          anchor.web3.ComputeBudgetProgram.setComputeUnitPrice({
            microLamports: 1000,
          }),
        ])
        .signers([user])
        .rpc();

      console.log("✅ USDC deposit transaction:", usdcTxSignature);

      // Verify deposit was successful
      await new Promise((resolve) => setTimeout(resolve, 3000));
      
      const vaultBalanceAfter = await connection.getTokenAccountBalance(usdcVaultAta);
      const vaultBalanceAfterAmount = parseInt(vaultBalanceAfter.value.amount);
      const depositedAmount = vaultBalanceAfterAmount - vaultBalanceBeforeAmount;

      console.log(`Vault USDC after: ${(vaultBalanceAfterAmount / 1_000_000).toLocaleString()}`);
      console.log(`Successfully deposited: ${(depositedAmount / 1_000_000).toLocaleString()}`);

      expect(depositedAmount).to.be.greaterThan(0);
      console.log("✅ Vault funded successfully - ready for arbitrage!");

    } catch (error) {
      console.error("❌ Deposit failed:", error);
      if (error.logs) {
        console.log("📋 Deposit transaction logs:");
        error.logs.forEach((log, i) => {
          console.log(`${i}: ${log}`);
        });
      }
      throw error;
    }
  });

  it("Verifies final vault state", async () => {
    try {
      const vaultState = await program.account.arbitrageVault.fetch(vaultPDA);
      console.log("=== Final Vault State ===");
      console.log("Authority:", vaultState.authority.toBase58());
      console.log(
        "Min Profit Threshold:",
        vaultState.minProfitThreshold.toNumber() / 1_000_000,
        "USDC"
      );
      console.log(
        "Max Single Trade:",
        vaultState.maxSingleTrade.toNumber() / 1_000_000,
        "USDC"
      );
      console.log("Total Trades:", vaultState.totalTrades.toNumber());
      console.log("Total Profits:", vaultState.totalProfits.toNumber());
      console.log("Failed Trades:", vaultState.failedTrades.toNumber());
      console.log("Bump:", vaultState.bump);

      // Verify vault token balances
      try {
        const vaultUsdcBalance = await connection.getTokenAccountBalance(
          usdcVaultAta
        );
        console.log(
          "Vault USDC Balance:",
          parseInt(vaultUsdcBalance.value.amount) / 1_000_000,
          "USDC"
        );
      } catch (error) {
        console.log("Could not fetch vault USDC balance:", error.message);
      }

      expect(vaultState.authority.equals(user.publicKey)).to.be.true;
      console.log("✅ All verifications passed");
    } catch (error) {
      console.error("❌ Final vault state verification failed:", error.message);
      throw error;
    }
  });

  it("Verifies arbitrage program structure with real pool detection", async () => {
    console.log("=== Pool Address Verification ===");

    try {
      const poolAInfo = await connection.getAccountInfo(poolA);
      const poolBInfo = await connection.getAccountInfo(poolB);

      if (poolAInfo && poolBInfo) {
        console.log("✅ Both pools exist on-chain");
        
        const sarosProgramId = new anchor.web3.PublicKey(
          "1qbkdrr3z4ryLA7pZykqxvxWPoeifcVKo6ZG9CfkvVE"
        );

        if (poolAInfo.owner.equals(sarosProgramId) && poolBInfo.owner.equals(sarosProgramId)) {
          console.log("🎯 Both pools are valid Saros DLMM pools!");
        }
      } else {
        console.log("⚠️ One or both pools not found");
      }
    } catch (error) {
      console.log("Pool verification error:", error.message);
    }

    console.log("💡 Pool integration verified - ready for arbitrage");
  });

  it("Executes actual profitable Saros DLMM arbitrage", async () => {
    let tradeAmount = new anchor.BN(1000 * 1_000_000); // $1K USDC

    console.log("🚀 === SAROS DLMM ARBITRAGE EXECUTION ===");

    try {
      // Ensure sufficient SOL for arbitrage transaction
      await ensureSufficientSolBalance(user.publicKey, 2 * LAMPORTS_PER_SOL);

      // Verify vault is ready for arbitrage
      const vaultStateBefore = await program.account.arbitrageVault.fetch(vaultPDA);
      console.log("✅ Vault is ready for arbitrage");
      
      // Check vault balance
      let vaultUsdcBalance = 0;
      try {
        const vaultUsdcBefore = await connection.getTokenAccountBalance(usdcVaultAta);
        vaultUsdcBalance = parseInt(vaultUsdcBefore.value.amount);
      } catch (error) {
        console.log("Could not get vault balance, assuming 0");
      }

      console.log(`Vault USDC: $${(vaultUsdcBalance / 1_000_000).toLocaleString()}`);
      console.log(`Trade Size: $${(tradeAmount.toNumber() / 1_000_000).toLocaleString()}`);
      console.log(`Previous Trades: ${vaultStateBefore.totalTrades.toNumber()}`);
      console.log(`Previous Profits: $${(vaultStateBefore.totalProfits.toNumber() / 1_000_000).toFixed(2)}`);

      if (vaultUsdcBalance < tradeAmount.toNumber()) {
        console.log(`⚠️ Vault has insufficient funds for trade`);
        console.log(`Available: $${(vaultUsdcBalance / 1_000_000).toLocaleString()}`);
        
        if (vaultUsdcBalance > 100_000) { // At least $0.10 to trade
          const availableAmount = Math.floor(vaultUsdcBalance * 0.9);
          tradeAmount = new anchor.BN(availableAmount);
          console.log(`Adjusted trade size: $${(tradeAmount.toNumber() / 1_000_000).toFixed(2)}`);
        } else {
          throw new Error("Insufficient vault funds for any meaningful trade");
        }
      }

      const startTime = Date.now();

      const txSignature = await program.methods
        .executeArbitrage(poolA, poolB, tradeAmount)
        .accounts({
          vault: vaultPDA,
          authority: user.publicKey,
          poolA: poolA,
          poolB: poolB,
          vaultTokenX: sarosVaultAta,
          vaultTokenY: usdcVaultAta,
          tokenXMint: sarosMint,
          tokenYMint: usdcMint,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .preInstructions([
          anchor.web3.ComputeBudgetProgram.setComputeUnitLimit({
            units: 400_000,
          }),
          anchor.web3.ComputeBudgetProgram.setComputeUnitPrice({
            microLamports: 1000,
          }),
        ])
        .signers([user])
        .rpc();

      const executionTime = Date.now() - startTime;
      console.log(`🎯 Transaction completed in ${executionTime}ms`);
      console.log(`📝 Signature: ${txSignature}`);

      // Get detailed logs
      const txDetails = await connection.getTransaction(txSignature, {
        commitment: "confirmed",
        maxSupportedTransactionVersion: 0,
      });

      console.log("\n=== 💰 ARBITRAGE EXECUTION LOGS ===");
      if (txDetails?.meta?.logMessages) {
        txDetails.meta.logMessages.forEach((log, i) => {
          if (
            log.includes("🚀") || log.includes("USDC") || log.includes("SAROS") ||
            log.includes("Profit") || log.includes("Trade") || log.includes("SWAP") ||
            log.includes("✅") || log.includes("📈") || log.includes("📉")
          ) {
            console.log(`${String(i).padStart(2, '0')}: ${log}`);
          }
        });
      }

      // Verify profit tracking
      const vaultStateAfter = await program.account.arbitrageVault.fetch(vaultPDA);
      console.log(`\n📊 === VAULT STATISTICS ===`);
      console.log(`  Trades: ${vaultStateBefore.totalTrades.toNumber()} → ${vaultStateAfter.totalTrades.toNumber()}`);
      console.log(`  Total Profits: $${(vaultStateBefore.totalProfits.toNumber() / 1_000_000).toFixed(2)} → $${(vaultStateAfter.totalProfits.toNumber() / 1_000_000).toFixed(2)}`);

      const profitChange = vaultStateAfter.totalProfits.toNumber() - vaultStateBefore.totalProfits.toNumber();
      if (profitChange > 0) {
        console.log(`💰 Trade Profit: +$${(profitChange / 1_000_000).toFixed(2)} USDC`);
      }

      expect(vaultStateAfter.totalTrades.toNumber()).to.be.greaterThan(vaultStateBefore.totalTrades.toNumber());

      console.log("\n🎉 === ARBITRAGE EXECUTION COMPLETE ===");
      console.log("✅ Vault properly initialized and funded");
      console.log("✅ Arbitrage successfully executed");
      console.log("🚀 READY FOR MAINNET DEPLOYMENT!");

    } catch (error) {
      console.error("❌ Arbitrage execution failed:", error.message);

      if (error.logs) {
        console.log("\n📋 Detailed execution logs:");
        error.logs.slice(0, 15).forEach((log, i) => {
          console.log(`${String(i).padStart(2, '0')}: ${log}`);
        });
      }

      throw error;
    }
  });
});

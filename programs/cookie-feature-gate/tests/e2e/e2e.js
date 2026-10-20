// End-to-end test of cookie-feature-gate against a local test validator.
// Usage: node e2e.js <upgrade-authority-keypair.json> <feature F> <feature G> <cookie|stock>
//   F is activated and must switch on at the next epoch on the Cookie runtime
//   (and must NOT on stock Agave). G is activated then revoked.
const fs = require('fs');
const {
  Connection, Keypair, PublicKey, Transaction, TransactionInstruction,
  SystemProgram, LAMPORTS_PER_SOL, sendAndConfirmTransaction,
} = require('@solana/web3.js');

const PROGRAM = new PublicKey('CookmMGfe9ti8mrVkCFuZZsuAvjMSRagqh4t8Yxx4Ub4');
const FEATURE_PROG = new PublicKey('Feature111111111111111111111111111111111111');
const LOADER = new PublicKey('BPFLoaderUpgradeab1e11111111111111111111111');
const INCINERATOR = new PublicKey('1nc1nerator11111111111111111111111111111111');

const [, , uaPath, fArg, gArg, mode] = process.argv;
const F = new PublicKey(fArg);
const G = new PublicKey(gArg);
const upgradeAuth = Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(uaPath))));
const authority = Keypair.generate();
const authority2 = Keypair.generate();
const intruder = Keypair.generate();
const conn = new Connection('http://127.0.0.1:8899', 'confirmed');

const pda = (seeds) => PublicKey.findProgramAddressSync(seeds, PROGRAM)[0];
const CONFIG = pda([Buffer.from('config')]);
const PROGRAM_DATA = PublicKey.findProgramAddressSync([PROGRAM.toBuffer()], LOADER)[0];
const alias = (f) => pda([Buffer.from('cookie-alias'), f.toBuffer()]);
const data = (tag, key) => Buffer.concat([Buffer.from([tag]), key.toBuffer()]);
const m = (pubkey, isSigner, isWritable) => ({ pubkey, isSigner, isWritable });
const SYS = SystemProgram.programId;

const ix = {
  init: (payer, ua, auth) => new TransactionInstruction({ programId: PROGRAM, data: data(0, auth),
    keys: [m(CONFIG, false, true), m(payer, true, true), m(ua, true, false), m(PROGRAM_DATA, false, false), m(SYS, false, false)] }),
  setAuth: (auth, next) => new TransactionInstruction({ programId: PROGRAM, data: data(1, next),
    keys: [m(CONFIG, false, true), m(auth, true, false)] }),
  activate: (auth, payer, f) => new TransactionInstruction({ programId: PROGRAM, data: data(2, f),
    keys: [m(CONFIG, false, false), m(auth, true, false), m(alias(f), false, true), m(payer, true, true), m(SYS, false, false)] }),
  revoke: (auth, f) => new TransactionInstruction({ programId: PROGRAM, data: data(3, f),
    keys: [m(CONFIG, false, false), m(auth, true, false), m(alias(f), false, true), m(INCINERATOR, false, true), m(FEATURE_PROG, false, false), m(SYS, false, false)] }),
};

let failures = 0;
const ok = (name) => console.log(`PASS  ${name}`);
const bad = (name, why) => { failures++; console.log(`FAIL  ${name}: ${why}`); };
const send = (ixs, signers) => sendAndConfirmTransaction(conn, new Transaction().add(...ixs), signers);

async function expectOk(name, ixs, signers) {
  try { await send(ixs, signers); ok(name); } catch (e) { bad(name, (e.logs || [e.message]).slice(-3).join(' | ')); }
}
async function expectFail(name, ixs, signers) {
  try { await send(ixs, signers); bad(name, 'transaction succeeded but should have failed'); }
  catch (e) { ok(`${name} (rejected)`); }
}
async function airdrop(pk, sol) {
  const sig = await conn.requestAirdrop(pk, sol * LAMPORTS_PER_SOL);
  await conn.confirmTransaction(sig, 'confirmed');
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

(async () => {
  console.log(`mode=${mode} F=${F.toBase58()} G=${G.toBase58()}`);
  console.log(`alias(F)=${alias(F).toBase58()} alias(G)=${alias(G).toBase58()}`);
  for (const k of [upgradeAuth, authority, authority2, intruder]) await airdrop(k.publicKey, 10);

  // Initialize: only the program's upgrade authority may set the first authority.
  await expectFail('initialize by someone other than the upgrade authority',
    [ix.init(intruder.publicKey, intruder.publicKey, intruder.publicKey)], [intruder]);
  await expectOk('initialize by the upgrade authority',
    [ix.init(upgradeAuth.publicKey, upgradeAuth.publicKey, authority.publicKey)], [upgradeAuth]);
  const cfg = await conn.getAccountInfo(CONFIG);
  if (cfg && cfg.owner.equals(PROGRAM) && cfg.data.length === 33 && cfg.data[0] === 1
      && new PublicKey(cfg.data.subarray(1, 33)).equals(authority.publicKey)) ok('config stores the authority');
  else bad('config stores the authority', JSON.stringify(cfg && { owner: cfg.owner.toBase58(), len: cfg.data.length }));
  await expectFail('initialize a second time',
    [ix.init(upgradeAuth.publicKey, upgradeAuth.publicKey, intruder.publicKey)], [upgradeAuth]);

  // Activate F.
  await expectFail('activate signed by a non-authority',
    [ix.activate(intruder.publicKey, intruder.publicKey, F)], [intruder]);
  await expectOk('activate F by the authority', [ix.activate(authority.publicKey, authority.publicKey, F)], [authority]);
  const aF = await conn.getAccountInfo(alias(F));
  if (aF && aF.owner.equals(FEATURE_PROG) && aF.data.length === 9 && aF.data.every((b) => b === 0))
    ok('alias(F) is a pending feature account (owner Feature111, 9 zero bytes)');
  else bad('alias(F) is a pending feature account', JSON.stringify(aF && { owner: aF.owner.toBase58(), data: [...aF.data] }));
  await expectFail('activate F again', [ix.activate(authority.publicKey, authority.publicKey, F)], [authority]);

  // Someone pre-funds alias(G) to try to block it; activation must still work.
  await expectOk('attacker sends lamports to alias(G)',
    [SystemProgram.transfer({ fromPubkey: intruder.publicKey, toPubkey: alias(G), lamports: 900000 })], [intruder]);
  await expectOk('activate G despite the pre-funded address', [ix.activate(authority.publicKey, authority.publicKey, G)], [authority]);
  const aG = await conn.getAccountInfo(alias(G));
  const pre = 900000; if (aG && aG.owner.equals(FEATURE_PROG) && aG.data.length === 9 && aG.lamports > pre) ok('alias(G) is a pending feature account, topped up to rent exemption');
  else bad('alias(G) is a pending feature account', JSON.stringify(aG && { owner: aG.owner.toBase58() }));

  // Revoke G before the epoch boundary.
  await expectFail('revoke signed by a non-authority', [ix.revoke(intruder.publicKey, G)], [intruder]);
  await expectOk('revoke G by the authority', [ix.revoke(authority.publicKey, G)], [authority]);
  const aG2 = await conn.getAccountInfo(alias(G));
  if (!aG2 || !aG2.owner.equals(FEATURE_PROG)) ok('alias(G) no longer a feature account after revoke');
  else bad('alias(G) no longer a feature account after revoke', 'still owned by Feature111');

  // Authority hand-over.
  await expectFail('set authority signed by a non-authority',
    [ix.setAuth(intruder.publicKey, intruder.publicKey)], [intruder]);
  await expectOk('set authority to a new key', [ix.setAuth(authority.publicKey, authority2.publicKey)], [authority]);
  await expectFail('old authority can no longer activate',
    [ix.activate(authority.publicKey, authority.publicKey, G)], [authority]);

  // Epoch boundary: the Cookie runtime must activate F through its alias.
  const start = await conn.getEpochInfo();
  console.log(`waiting for epoch ${start.epoch + 1} (slot ${start.absoluteSlot}, ${start.slotsInEpoch - start.slotIndex} slots left)`);
  for (;;) { await sleep(2000); if ((await conn.getEpochInfo()).epoch > start.epoch) break; }
  await sleep(3000);
  const after = await conn.getAccountInfo(alias(F));
  const activated = after && after.data[0] === 1;
  const slot = activated ? Number(after.data.readBigUInt64LE(1)) : null;
  if (mode === 'cookie') {
    if (activated) ok(`Cookie runtime activated F through its alias at slot ${slot}`);
    else bad('Cookie runtime activated F through its alias', `alias data ${after && [...after.data]}`);
  } else {
    if (!activated) ok('stock Agave ignores the alias (F stays pending)');
    else bad('stock Agave ignores the alias', `activated at ${slot}`);
  }
  const upstreamF = await conn.getAccountInfo(F);
  if (!upstreamF) ok('no upstream feature account was created for F');
  else bad('no upstream feature account was created for F', 'upstream account exists');
  const aG3 = await conn.getAccountInfo(alias(G));
  if (!aG3 || aG3.data.length < 1 || aG3.data[0] === 0 || !aG3.owner.equals(FEATURE_PROG)) ok('revoked G did not activate');
  else bad('revoked G did not activate', 'G alias shows activation');

  console.log(failures === 0 ? 'ALL CHECKS PASSED' : `${failures} CHECK(S) FAILED`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((e) => { console.error(e); process.exit(2); });

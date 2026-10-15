// Run a Cloaked Go-WASM encryption module locally (Node 22+) and print test vectors
// for fixed, non-secret inputs. The module makes no network calls; it is loaded with
// Cloaked's own Go runtime shim from the web app's encryption.js.
//
//   node scripts/vectors.mjs artifacts/web/encryption.js artifacts/web/encryption.wasm
//   node scripts/vectors.mjs artifacts/web/encryption.js artifacts/extension/src/encryption.wasm
import fs from 'node:fs';
import vm from 'node:vm';

const [shimPath, wasmPath] = process.argv.slice(2);
if (!shimPath || !wasmPath) {
  console.error('usage: node vectors.mjs <encryption.js> <encryption.wasm>');
  process.exit(2);
}

globalThis.self = globalThis;
globalThis.window = globalThis;
// encryption.js is a UMD bundle that mentions import.meta; it is not used on this path.
vm.runInThisContext(fs.readFileSync(shimPath, 'utf8').replaceAll('import.meta.url', '"file:///encryption.js"'));

const go = new globalThis.Go();
const { instance } = await WebAssembly.instantiate(fs.readFileSync(wasmPath), go.importObject);
go.run(instance);
await new Promise((resolve) => setTimeout(resolve, 200)); // let main() register its exports

const lib = globalThis.cloakedEncryption;
const accountSalt = Buffer.from([...Array(32).keys()]).toString('base64url'); // bytes 00..1f
const password = 'correct horse battery staple';
const secretBoxKey = await lib.generatePasswordSecretBoxKey(password, accountSalt);
const authKey = await lib.generatePasswordAuthKey(secretBoxKey, accountSalt);

console.log(JSON.stringify({
  password,
  account_salt: accountSalt,
  secret_box_key: secretBoxKey,
  auth_key: authKey,
}, null, 2));
process.exit(0);

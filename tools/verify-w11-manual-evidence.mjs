// Local release-review helper: compare a manual manifest with candidate bytes.
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkManualGateCoverage } from './check-w11-manual-gates.mjs';

async function hashExecutable(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

export async function verifyManualEvidence(manifestPath, executablePath) {
  const [manifestText, executableSha256] = await Promise.all([
    readFile(manifestPath, 'utf8'),
    hashExecutable(executablePath)
  ]);
  const manifest = JSON.parse(manifestText);
  const gates = Array.isArray(manifest) ? manifest : manifest?.gates;
  return {
    ...checkManualGateCoverage(gates, { expectedExecutableSha256: executableSha256 }),
    executableSha256
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2);
  if (args.length !== 2) {
    console.error('Usage: node tools/verify-w11-manual-evidence.mjs <manifest.json> <fedorawin.exe>');
    process.exitCode = 2;
  } else {
    try {
      const result = await verifyManualEvidence(args[0], args[1]);
      console.log(JSON.stringify(result, null, 2));
      console.log('STRUCTURAL CHECK ONLY: beta approval requires human Windows 11 review.');
      if (!result.complete) process.exitCode = 1;
    } catch (error) {
      console.error('Manual evidence verification failed:', error.message);
      process.exitCode = 1;
    }
  }
}

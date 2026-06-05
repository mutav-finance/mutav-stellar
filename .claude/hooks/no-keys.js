#!/usr/bin/env node
// PreToolUse hook — hard-block edits introducing signing-key custody into mutav-stellar/src/.
// The SDK composes XDRs only; consumers sign. Test fixtures are exempt (excluded by path).
// See: mutav-stellar/CLAUDE.md "Boundary rule", workspace CLAUDE.md "## Must never".

const PATTERNS = [
  { name: 'Keypair.fromSecret', re: /Keypair\.fromSecret\(/ },
  { name: 'StrKey.decodeEd25519SecretSeed', re: /StrKey\.decodeEd25519SecretSeed\(/ },
  { name: 'secret-named env read', re: /process\.env\.[A-Z_]*(SECRET|PRIVATE|SEED)\b/ },
];

function isInScope(filePath) {
  if (!filePath.includes('/src/')) return false;
  if (filePath.includes('/__tests__/')) return false;
  if (/\.test\.tsx?$/.test(filePath)) return false;
  if (!/\.tsx?$/.test(filePath)) return false;
  return true;
}

let input = '';
const stdinTimeout = setTimeout(() => process.exit(0), 3000);
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => (input += chunk));
process.stdin.on('end', () => {
  clearTimeout(stdinTimeout);
  try {
    const data = JSON.parse(input);
    if (!['Edit', 'Write', 'MultiEdit'].includes(data.tool_name)) {
      process.exit(0);
    }
    const filePath = data.tool_input?.file_path || '';
    if (!isInScope(filePath)) {
      process.exit(0);
    }
    const content = data.tool_input?.content || data.tool_input?.new_string || '';
    if (!content) process.exit(0);

    const hits = PATTERNS.filter(p => p.re.test(content)).map(p => p.name);
    if (hits.length === 0) process.exit(0);

    process.stderr.write(
      `[no-keys] This edit introduces signing-key custody into mutav-stellar/src/.\n` +
        `Matched: ${hits.join(', ')}.\n` +
        `The SDK composes XDRs only — consumers sign.\n` +
        `See: mutav-stellar/CLAUDE.md "Boundary rule", workspace CLAUDE.md "## Must never".\n` +
        `If this is a test fixture, place it under src/**/__tests__/ or name it *.test.ts (excluded).\n`
    );
    process.exit(2);
  } catch {
    process.exit(0);
  }
});

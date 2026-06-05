#!/usr/bin/env node
// PreToolUse hook — block edits to vendored branding/ files.
// Files under branding/ come from the brand/ source-of-truth repo via bun brand:export.
// In-place edits here create silent drift that bun brand:audit will report.

let input = '';
const stdinTimeout = setTimeout(() => process.exit(0), 3000);
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => (input += chunk));
process.stdin.on('end', () => {
  clearTimeout(stdinTimeout);
  try {
    const data = JSON.parse(input);
    const toolName = data.tool_name;
    if (!['Edit', 'Write', 'MultiEdit'].includes(toolName)) {
      process.exit(0);
    }
    const filePath = data.tool_input?.file_path || '';
    if (!filePath.includes('/branding/')) {
      process.exit(0);
    }
    process.stderr.write(
      '[no-brand-edit] branding/ files are vendored from brand/.\n' +
        'To update: cd ../brand && bun brand:import <this-repo>  then  bun brand:export\n' +
        'See .claude/rules/brand.md.\n'
    );
    process.exit(2);
  } catch {
    process.exit(0);
  }
});

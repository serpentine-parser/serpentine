## UI Verification

Use the Bash tool to run this playwright script to capture a screenshot AND console errors:

```bash
node frontend/ui-check.mjs
```

(`ui-check.mjs` lives in `frontend/` alongside its `playwright` dep. The URL defaults to `http://localhost:5173`; pass `$ARGUMENTS` to override.)

Then:

1. Read `/tmp/ui-check.png` to visually inspect the result
2. Review the ERRORS and WARNINGS output
3. Check for: console errors, layout breaks, missing content, obvious visual regressions
4. Report findings with element references (file:line), not just descriptions
5. If issues found, fix them and re-verify before declaring done

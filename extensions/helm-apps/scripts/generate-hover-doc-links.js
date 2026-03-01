const fs = require("node:fs");
const path = require("node:path");
const { extractDocLinks } = require("./hover-doc-links-lib");

const extRoot = path.resolve(__dirname, "..");
const sourcePath = path.join(extRoot, "src", "hover", "fieldHover.ts");
const outPath = path.join(extRoot, "docs", "hover-doc-links.json");

const sourceText = fs.readFileSync(sourcePath, "utf8");
const links = extractDocLinks(sourceText);

fs.mkdirSync(path.dirname(outPath), { recursive: true });
fs.writeFileSync(outPath, `${JSON.stringify({ links }, null, 2)}\n`, "utf8");
// eslint-disable-next-line no-console
console.log(`generated ${path.relative(extRoot, outPath)} (${links.length} links)`);

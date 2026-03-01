const fs = require("node:fs");
const path = require("node:path");
const { extractDocLinks, validateDocLinks } = require("./hover-doc-links-lib");

const extRoot = path.resolve(__dirname, "..");
const sourcePath = path.join(extRoot, "src", "hover", "fieldHover.ts");
const catalogPath = path.join(extRoot, "docs", "hover-doc-links.json");

const sourceText = fs.readFileSync(sourcePath, "utf8");
const links = extractDocLinks(sourceText);

if (!fs.existsSync(catalogPath)) {
  throw new Error("missing docs/hover-doc-links.json, run: npm run generate:hover-docs");
}
const catalogRaw = fs.readFileSync(catalogPath, "utf8");
const catalog = JSON.parse(catalogRaw);
const catalogLinks = Array.isArray(catalog.links) ? [...catalog.links].sort() : [];

if (JSON.stringify(links) !== JSON.stringify(catalogLinks)) {
  throw new Error("hover docs catalog is out of date, run: npm run generate:hover-docs");
}

const { errors, warnings } = validateDocLinks(extRoot, links);
if (errors.length > 0) {
  throw new Error(`hover docs validation failed:\n- ${errors.join("\n- ")}`);
}
if (warnings.length > 0) {
  // eslint-disable-next-line no-console
  console.warn(`hover docs warnings:\n- ${warnings.join("\n- ")}`);
}

// eslint-disable-next-line no-console
console.log(`hover docs links OK (${links.length})`);

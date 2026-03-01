const fs = require("node:fs");
const path = require("node:path");

function extractDocLinks(sourceText) {
  const out = new Set();
  const re = /\bdocsLink(?:En|Ru)?\s*:\s*"([^"]+)"/g;
  for (const m of sourceText.matchAll(re)) {
    const link = (m[1] || "").trim();
    if (link.length > 0) {
      out.add(link);
    }
  }
  return [...out].sort();
}

function githubSlug(s) {
  return s
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

function collectAnchors(markdown) {
  const anchors = new Set();
  const counts = new Map();
  const headingRe = /^(#{1,6})\s+(.+?)\s*$/gm;
  for (const m of markdown.matchAll(headingRe)) {
    const raw = m[2].trim();
    const base = githubSlug(raw);
    const n = counts.get(base) ?? 0;
    counts.set(base, n + 1);
    anchors.add(n === 0 ? base : `${base}-${n}`);
  }
  return anchors;
}

function validateDocLinks(projectRoot, links) {
  const errors = [];
  const warnings = [];
  for (const link of links) {
    if (/^https?:\/\//.test(link)) {
      continue;
    }
    if (!link.startsWith("docs/")) {
      errors.push(`unsupported docsLink '${link}' (expected docs/* or http(s) link)`);
      continue;
    }
    const [filePart, anchorPart] = link.split("#");
    const abs = path.resolve(projectRoot, "..", "..", filePart);
    if (!fs.existsSync(abs)) {
      errors.push(`docsLink target file not found: ${filePart}`);
      continue;
    }
    if (!anchorPart) {
      continue;
    }
    const markdown = fs.readFileSync(abs, "utf8");
    const anchors = collectAnchors(markdown);
    if (!anchors.has(anchorPart)) {
      const hasExplicitAnchor = markdown.includes(`id="${anchorPart}"`) || markdown.includes(`{#${anchorPart}}`);
      if (!hasExplicitAnchor) {
        warnings.push(`docsLink anchor not found: ${link}`);
      }
    }
  }
  return { errors, warnings };
}

module.exports = {
  extractDocLinks,
  validateDocLinks,
};

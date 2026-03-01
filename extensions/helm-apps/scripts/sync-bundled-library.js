const fs = require("node:fs");
const path = require("node:path");

const extRoot = path.resolve(__dirname, "..");
const repoRoot = path.resolve(extRoot, "..", "..");
const src = path.join(repoRoot, "charts", "helm-apps");
const dst = path.join(extRoot, "assets", "helm-apps");

function rmrf(p) {
  if (!fs.existsSync(p)) return;
  for (const name of fs.readdirSync(p)) {
    const full = path.join(p, name);
    const st = fs.statSync(full);
    if (st.isDirectory()) {
      rmrf(full);
    } else {
      fs.unlinkSync(full);
    }
  }
  fs.rmdirSync(p);
}

function cpdir(from, to) {
  fs.mkdirSync(to, { recursive: true });
  for (const name of fs.readdirSync(from)) {
    const s = path.join(from, name);
    const d = path.join(to, name);
    const st = fs.statSync(s);
    if (st.isDirectory()) {
      cpdir(s, d);
    } else {
      fs.copyFileSync(s, d);
    }
  }
}

if (!fs.existsSync(src)) {
  throw new Error(`source chart not found: ${src}`);
}

rmrf(dst);
cpdir(src, dst);
// eslint-disable-next-line no-console
console.log(`synced bundled helm-apps chart: ${dst}`);

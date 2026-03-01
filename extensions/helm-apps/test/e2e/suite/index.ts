import path from "node:path";
import Mocha from "mocha";

export function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
    timeout: 30000,
  });
  mocha.addFile(path.resolve(__dirname, "smoke.test.js"));
  return new Promise((resolve, reject) => {
    mocha.run((failures) => {
      if (failures > 0) {
        reject(new Error(`${failures} e2e tests failed.`));
      } else {
        resolve();
      }
    });
  });
}

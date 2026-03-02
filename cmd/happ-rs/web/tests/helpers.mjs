export async function stabilizePage(page) {
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        transition: none !important;
        animation: none !important;
        caret-color: transparent !important;
      }
      .cm-cursor, .sync-cursor, .happ-virtual-cursor { opacity: 0 !important; }
    `,
  });
}

export async function openUtility(page, tabTitle) {
  await page.getByRole("button", { name: tabTitle }).click();
  await page.waitForLoadState("networkidle");
}

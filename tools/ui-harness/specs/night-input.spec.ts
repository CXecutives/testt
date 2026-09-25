// The press that ends the OS autoscroll ends only the autoscroll: it presses nothing,
// whichever button it is, like the native scrolling of Windows.

import { expect, open, test } from './fixtures';

test('the click that ends the autoscroll presses nothing; a dragged middle press leaves no mode', async ({
  page,
  browserName,
}) => {
  // The autoscroll is Windows' (WebView2, a Chromium): WebKit has none to end.
  test.skip(browserName === 'webkit', 'no autoscroll in WebKit');
  await open(page, '?platform=windows&scenario=many');
  const all = page.getByTestId('facet').getByRole('radio', { name: /Alle/ });
  const list = (await page.getByTestId('job-list').boundingBox())!;
  const inList = { x: list.x + list.width / 2, y: list.y + list.height / 2 };
  // A middle click in the list: the autoscroll runs until the next press.
  await page.mouse.move(inList.x, inList.y);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.up({ button: 'middle' });
  await all.click();
  await expect(all).not.toHaveAttribute('aria-checked', 'true');
  // The next click is an ordinary one again.
  await all.click();
  await expect(all).toHaveAttribute('aria-checked', 'true');
  // Held and dragged, the middle button scrolled while held: no mode is left.
  const fresh = page.getByTestId('facet').getByRole('radio', { name: /Neu/ });
  await page.mouse.move(inList.x, inList.y);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(inList.x, inList.y + 60, { steps: 4 });
  await page.mouse.up({ button: 'middle' });
  await fresh.click();
  await expect(fresh).toHaveAttribute('aria-checked', 'true');
});

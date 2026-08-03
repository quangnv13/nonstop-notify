import { expect, test } from '@playwright/test';

const toasterBottomPadding = 24;

const toasts = Array.from({ length: 5 }, (_, index) => ({
  id: `toast-${index}`,
  title: index % 2 ? 'Đang chạy test suite' : 'Đang chạy test',
  message: index === 1
    ? 'Tạo policy Craftinsure cho subproduct CruisingYacht mới dùng policyholder mới hoàn toàn'
    : `Tạo policyholder individual trên Juice ${index}`,
  state: 'loading',
  progress: 0,
  route: '/',
  primaryLabel: 'Mở dashboard',
  primaryRoute: '/',
  secondaryLabel: '',
  secondaryRoute: '',
  sticky: true,
}));

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const reports: Array<{ width: number; height: number }> = [];
    const commands: string[] = [];
    Object.assign(window, {
      __layoutReports: reports,
      __invokeCommands: commands,
      __TAURI_INTERNALS__: {
        invoke: async (cmd: string, args?: { width: number; height: number }) => {
          commands.push(cmd);
          if (cmd === 'report_layout' && args) reports.push(args);
        },
      },
    });
  });
  await page.goto('/');
});

test('dismissible toast schedules auto close after 6000ms', async ({ page }) => {
  await page.evaluate(() => {
    const scheduledTimeouts: number[] = [];
    const originalSetTimeout = window.setTimeout.bind(window);
    (window as unknown as { __scheduledTimeouts: number[] }).__scheduledTimeouts = scheduledTimeouts;
    window.setTimeout = ((handler: TimerHandler, timeout?: number, ...args: unknown[]) => {
      scheduledTimeouts.push(timeout ?? 0);
      return originalSetTimeout(handler, timeout, ...args);
    }) as typeof window.setTimeout;
  });
  await page.evaluate((item) => window.__NONSTOP_SET_TOASTS?.({
    toasts: [{ ...item, id: 'toast-timeout', state: 'success', sticky: false }],
    expanded: false,
  }), toasts[0]);

  await expect.poll(() => page.evaluate(() => (window as unknown as { __scheduledTimeouts: number[] }).__scheduledTimeouts)).toContain(6_000);
});

test('success toast with two actions never reports its entering transform height', async ({ page }) => {
  const successToast = {
    ...toasts[0],
    id: 'toast-success-two-actions',
    title: 'Test case chạy thành công',
    message: 'Tạo policyholder individual trên Juice',
    state: 'success',
    primaryLabel: 'Mở dashboard',
    primaryRoute: '/',
    secondaryLabel: 'Xem chi tiết',
    secondaryRoute: '/runs/manual',
    sticky: false,
  };
  await page.setViewportSize({ width: 430, height: 150 });
  await clearLayoutReports(page);
  await page.evaluate((item) => window.__NONSTOP_SET_TOASTS?.({ toasts: [item], expanded: false, theme: 'dark' }), successToast);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(1);
  await page.waitForTimeout(250);

  const layout = await page.evaluate((bottomPadding) => {
    const toast = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')!;
    const intrinsicHeight = Number.parseFloat(getComputedStyle(toast).getPropertyValue('--initial-height')) || toast.offsetHeight;
    const reports = (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports;
    return {
      minimumHeight: Math.ceil(toast.offsetTop + intrinsicHeight + bottomPadding),
      reports: reports.map(({ height }) => height),
    };
  }, toasterBottomPadding);

  expect(layout.reports.length).toBeGreaterThan(0);
  expect(layout.reports.every((height) => height >= layout.minimumHeight)).toBe(true);
});

test('window height includes front toast content when its box height is stale', async ({ page }) => {
  const longToast = {
    ...toasts[0],
    id: 'toast-stale-height',
    message: 'Nội dung dài hơn bình thường một chút, xuống thêm dòng và có đủ action để kiểm tra phần đáy toast không bị cắt.',
    secondaryLabel: 'Xem chi tiết',
    secondaryRoute: '/runs/manual',
  };
  await page.evaluate((item) => window.__NONSTOP_SET_TOASTS?.({ toasts: [item], expanded: false }), longToast);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(1);
  await page.waitForTimeout(250);

  await page.locator('[data-sonner-toast][data-front="true"]').evaluate((element) => {
    const toast = element as HTMLElement;
    toast.style.transition = 'none';
    toast.style.height = Math.max(1, toast.getBoundingClientRect().height - 60) + 'px';
  });
  await page.waitForTimeout(150);

  const layout = await page.evaluate((bottomPadding) => {
    const toast = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')!;
    const content = toast.querySelector<HTMLElement>('.notify-body')!;
    const reports = (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports;
    return {
      contentBottom: Math.ceil(content.getBoundingClientRect().bottom + bottomPadding),
      reported: reports.at(-1)?.height ?? 0,
      toastBottom: Math.ceil(toast.getBoundingClientRect().bottom + bottomPadding),
    };
  }, toasterBottomPadding);

  expect(layout.contentBottom).toBeGreaterThan(layout.toastBottom);
  expect(layout.reported).toBeGreaterThanOrEqual(layout.contentBottom);
});

test('window height follows expanded and collapsed toast bounds', async ({ page }) => {
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({ toasts: items, expanded: false }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);

  const minimumExpandedHeight = await page.evaluate((bottomPadding) => {
    const toastHeights = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
      .map((element) => Number.parseFloat(getComputedStyle(element).getPropertyValue('--initial-height')) || element.offsetHeight);
    return Math.min(760, Math.ceil(toastHeights.reduce((total, height) => total + height, 0) + (toastHeights.length - 1) * 10 + bottomPadding));
  }, toasterBottomPadding);
  await page.evaluate(() => {
    (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.length = 0;
    (window as unknown as { __invokeCommands: string[] }).__invokeCommands.length = 0;
  });
  await page.locator('[data-sonner-toast]').first().hover();
  const immediateExpandedHeight = await page.evaluate(() => (
    (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.at(-1)?.height ?? 0
  ));
  expect(immediateExpandedHeight).toBeGreaterThanOrEqual(minimumExpandedHeight);
  const hoverState = await page.evaluate(() => ({
    commands: (window as unknown as { __invokeCommands: string[] }).__invokeCommands,
    transitionMs: Math.max(...getComputedStyle(document.querySelector<HTMLElement>('[data-sonner-toast]')!)
      .transitionDuration.split(',').map((value) => Number.parseFloat(value) * 1000)),
  }));
  expect(hoverState.commands).not.toContain('set_expanded');
  expect(hoverState.transitionMs).toBeLessThanOrEqual(180);
  await page.waitForTimeout(700);
  const expanded = await layoutSnapshot(page);
  expect(Math.abs(expanded.reported - expanded.visualBottom)).toBeLessThanOrEqual(2);

  await page.mouse.move(500, 800);
  await page.waitForTimeout(900);
  const collapsed = await layoutSnapshot(page);
  expect(Math.abs(collapsed.reported - collapsed.visualBottom)).toBeLessThanOrEqual(2);
  expect(collapsed.reported).toBeLessThan(expanded.reported / 2);
});

test('pre-grows when cursor waits in transparent edge before entering toast', async ({ page }) => {
  await page.mouse.move(5, 140);
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({ toasts: items, expanded: false }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await page.waitForTimeout(300);
  const minimumExpandedHeight = await expandedLayoutHeight(page);
  await page.evaluate(() => {
    const state = window as unknown as {
      __layoutReports: Array<{ height: number }>;
      __pointerEventHeight: number;
    };
    state.__layoutReports.length = 0;
    state.__pointerEventHeight = 0;
    document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')?.addEventListener('pointerover', () => {
      queueMicrotask(() => {
        state.__pointerEventHeight = state.__layoutReports.at(-1)?.height ?? 0;
      });
    }, { once: true });
  });
  await page.locator('[data-sonner-toast][data-front="true"]').hover();
  const pointerEventHeight = await page.evaluate(() => (
    (window as unknown as { __pointerEventHeight: number }).__pointerEventHeight
  ));
  expect(pointerEventHeight).toBeGreaterThanOrEqual(minimumExpandedHeight);
});

test('transparent wrapper edge does not request expanded height', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 190 });
  await page.mouse.move(500, 800);
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-left',
  }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await page.waitForTimeout(300);
  const minimumExpandedHeight = await expandedLayoutHeight(page);
  await clearLayoutReports(page);

  await page.mouse.move(425, 140);
  await page.waitForTimeout(100);

  const edgeReports = await page.evaluate(() => (
    window as unknown as { __layoutReports: Array<{ height: number }> }
  ).__layoutReports.map((report) => report.height));
  expect(edgeReports.every((height) => height < minimumExpandedHeight)).toBe(true);
});

test('collapsed rear toast invisible area never triggers expansion', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 190 });
  await page.mouse.move(500, 800);
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-left',
  }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await page.waitForTimeout(300);

  const point = await page.evaluate(() => {
    const front = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')!;
    const frontRect = front.getBoundingClientRect();
    const rearToasts = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast][data-front="false"]')];
    for (const rear of rearToasts) {
      const rect = rear.getBoundingClientRect();
      const top = Math.max(0, rect.top + 2);
      const bottom = Math.min(window.innerHeight - 1, rect.bottom - 2, frontRect.top - 2);
      if (bottom > top) return { x: Math.round((rect.left + rect.right) / 2), y: Math.round((top + bottom) / 2) };
    }
    return null;
  });
  if (!point) throw new Error('No invisible rear-toast point found');
  await clearLayoutReports(page);

  await page.mouse.move(point.x, point.y);
  await page.waitForTimeout(350);

  const state = await page.evaluate(() => ({
    expanded: document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')?.dataset.expanded,
    reports: (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports,
  }));
  expect(state.expanded).toBe('false');
  expect(state.reports).toEqual([]);
});

test('single bottom toast with five lines converges without clipping', async ({ page }) => {
  const longToast = {
    ...toasts[0],
    id: 'toast-bottom-five-lines',
    title: 'Thông báo nội dung dài cần hiển thị đầy đủ',
    message: 'Dòng thứ nhất.\nDòng thứ hai.\nDòng thứ ba.\nDòng thứ tư.\nDòng thứ năm.',
  };
  await page.setViewportSize({ width: 430, height: 150 });
  await page.evaluate((item) => window.__NONSTOP_SET_TOASTS?.({
    toasts: [item],
    expanded: false,
    position: 'bottom-left',
  }), longToast);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(1);

  const appliedHeights = await applyNativeResizeFeedback(page, 700);
  const layout = await page.evaluate((bottomPadding) => {
    const toast = document.querySelector<HTMLElement>('[data-sonner-toast]')!;
    const title = toast.querySelector<HTMLElement>('[data-title]')!;
    const message = toast.querySelector<HTMLElement>('.notify-message')!;
    return {
      requiredHeight: Math.ceil(toast.scrollHeight + bottomPadding),
      titleTop: title.getBoundingClientRect().top,
      messageTop: message.getBoundingClientRect().top,
    };
  }, toasterBottomPadding);

  expect(appliedHeights.length).toBeGreaterThan(0);
  expect(appliedHeights.every((height) => height >= layout.requiredHeight)).toBe(true);
  expect(layout.titleTop).toBeGreaterThanOrEqual(0);
  expect(layout.messageTop).toBeGreaterThanOrEqual(0);
});

test('native resize feedback never applies intermediate transition heights', async ({ page }) => {
  const feedbackToasts = toasts.map((item, index) => ({
    ...item,
    title: 'Đang chạy test',
    message: 'Toast ' + index,
  }));
  await page.setViewportSize({ width: 430, height: 190 });
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({ toasts: items, expanded: false }), feedbackToasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await page.waitForTimeout(300);

  const minimumExpandedHeight = await expandedLayoutHeight(page);
  await clearLayoutReports(page);
  await page.locator('[data-sonner-toast][data-front="true"]').hover();
  const expansionReports = await applyNativeResizeFeedback(page, 500);

  expect(expansionReports[0]).toBeGreaterThanOrEqual(minimumExpandedHeight);
  expect(expansionReports.every((height) => height >= minimumExpandedHeight)).toBe(true);

  const expandedCards = await page.locator('[data-sonner-toast]').evaluateAll((elements) => elements.map((element) => {
    const toast = element as HTMLElement;
    const action = toast.querySelector<HTMLElement>('.notify-action-primary');
    return {
      height: toast.getBoundingClientRect().height,
      initialHeight: Number.parseFloat(getComputedStyle(toast).getPropertyValue('--initial-height')) || toast.offsetHeight,
      actionHeight: action?.getBoundingClientRect().height ?? 0,
    };
  }));
  expandedCards.forEach((card) => expect(Math.abs(card.height - card.initialHeight)).toBeLessThanOrEqual(1));
  expect(Math.abs(expandedCards[0].actionHeight - expandedCards.at(-1)!.actionHeight)).toBeLessThanOrEqual(1);

  await clearLayoutReports(page);
  await page.mouse.move(500, 800);
  const collapseReports = await applyNativeResizeFeedback(page, 700);
  const collapsedHeight = collapseReports.at(-1) ?? 0;

  expect(collapsedHeight).toBeGreaterThan(0);
  expect(collapsedHeight).toBeLessThan(minimumExpandedHeight / 2);
  expect(new Set(collapseReports).size).toBe(1);
});

test('bottom collapsed layout reports visual stack height instead of viewport height', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 645 });
  await clearLayoutReports(page);
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-left',
  }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await page.waitForTimeout(300);

  const layout = await page.evaluate((bottomPadding) => {
    const rects = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
      .map((element) => element.getBoundingClientRect());
    const reports = (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports;
    return {
      visualHeight: Math.ceil(Math.max(...rects.map((rect) => rect.bottom)) - Math.min(...rects.map((rect) => rect.top)) + bottomPadding),
      reportedHeight: reports.at(-1)?.height ?? 0,
      viewportHeight: window.innerHeight,
    };
  }, toasterBottomPadding);

  expect(Math.abs(layout.reportedHeight - layout.visualHeight)).toBeLessThanOrEqual(2);
  expect(layout.reportedHeight).toBeLessThan(layout.viewportHeight / 2);

  const feedbackReports = await applyNativeResizeFeedback(page, 500);
  expect(feedbackReports).toEqual([layout.reportedHeight]);
});

test('bottom position anchors front toast and expands older toasts upward', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 190 });
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-right',
  }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  await expect(page.locator('[data-sonner-toaster]')).toHaveAttribute('data-y-position', 'bottom');
  await expect(page.locator('[data-sonner-toaster]')).toHaveAttribute('data-x-position', 'right');
  await page.waitForTimeout(300);

  const beforeBottomGap = await page.evaluate(() => {
    const front = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')!;
    return window.innerHeight - front.getBoundingClientRect().bottom;
  });
  const frontToast = page.locator('[data-sonner-toast][data-front="true"]');
  await frontToast.hover();
  await expect(frontToast).toHaveAttribute('data-expanded', 'true');
  const expandedHeight = await page.evaluate(() => (
    (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.at(-1)?.height ?? 0
  ));
  expect(expandedHeight).toBeGreaterThan(190);
  await page.setViewportSize({ width: 430, height: expandedHeight });
  await frontToast.hover();
  await expect(frontToast).toHaveAttribute('data-expanded', 'true');
  await page.waitForTimeout(250);

  const expandedLayout = await page.evaluate(() => {
    const front = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')!;
    const frontRect = front.getBoundingClientRect();
    const olderRects = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast][data-front="false"]')]
      .map((element) => element.getBoundingClientRect());
    return {
      bottomGap: window.innerHeight - frontRect.bottom,
      olderToastBottoms: olderRects.map((rect) => rect.bottom),
      frontTop: frontRect.top,
    };
  });

  expect(Math.abs(expandedLayout.bottomGap - beforeBottomGap)).toBeLessThanOrEqual(1);
  expect(expandedLayout.olderToastBottoms.length).toBeGreaterThan(0);
  expect(expandedLayout.olderToastBottoms.every((bottom) => bottom <= expandedLayout.frontTop + 1)).toBe(true);
});

test('new bottom toast payload keeps hovered stack expanded at full height', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 190 });
  await page.evaluate((item) => window.__NONSTOP_SET_TOASTS?.({
    toasts: [item],
    expanded: false,
    position: 'bottom-left',
  }), toasts[0]);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(1);
  const frontToast = page.locator('[data-sonner-toast][data-front="true"]');
  await frontToast.hover();
  await expect(frontToast).toHaveAttribute('data-expanded', 'true');

  const firstExpandedHeight = await expandedLayoutHeight(page);
  await page.setViewportSize({ width: 430, height: firstExpandedHeight });
  await frontToast.hover();
  await page.evaluate(() => {
    const transitions: Array<{ id: string; oldValue: string | null }> = [];
    const observer = new MutationObserver((records) => {
      records.forEach((record) => {
        const toast = record.target as HTMLElement;
        transitions.push({ id: toast.dataset.sonnerToast ?? '', oldValue: record.oldValue });
      });
    });
    observer.observe(document.body, {
      attributes: true,
      attributeFilter: ['data-expanded'],
      attributeOldValue: true,
      subtree: true,
    });
    Object.assign(window, { __expandedTransitions: transitions, __expandedObserver: observer });
  });
  await clearLayoutReports(page);
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-left',
  }), toasts.slice(0, 2));
  await page.waitForTimeout(20);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(2);
  const promptMinimumHeight = await expandedLayoutHeight(page);
  const promptReportedHeight = await page.evaluate(() => (
    (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.at(-1)?.height ?? 0
  ));
  expect(promptReportedHeight).toBeGreaterThanOrEqual(promptMinimumHeight);
  const currentFront = page.locator('[data-sonner-toast][data-front="true"]');
  await expect(currentFront).toHaveAttribute('data-expanded', 'true');
  await page.waitForTimeout(250);
  const expandedTransitions = await page.evaluate(() => {
    const state = window as unknown as {
      __expandedTransitions: Array<{ id: string; oldValue: string | null }>;
      __expandedObserver: MutationObserver;
    };
    state.__expandedObserver.disconnect();
    return state.__expandedTransitions;
  });
  expect(expandedTransitions.some(({ oldValue }) => oldValue === 'true')).toBe(false);

  const minimumExpandedHeight = await expandedLayoutHeight(page);
  const reports = await applyNativeResizeFeedback(page, 500);
  expect(reports.length).toBeGreaterThan(0);
  expect(reports.every((height) => height >= minimumExpandedHeight)).toBe(true);
});

test('spurious pointerleave during native resize keeps hovered bottom stack expanded', async ({ page }) => {
  await page.setViewportSize({ width: 430, height: 190 });
  await page.evaluate((items) => window.__NONSTOP_SET_TOASTS?.({
    toasts: items,
    expanded: false,
    position: 'bottom-left',
  }), toasts);
  await expect(page.locator('[data-sonner-toast]')).toHaveCount(5);
  const frontToast = page.locator('[data-sonner-toast][data-front="true"]');
  await frontToast.hover();
  await expect(frontToast).toHaveAttribute('data-expanded', 'true');

  await page.locator('.sonner-hitbox').dispatchEvent('pointerout', { relatedTarget: null });
  await page.waitForTimeout(350);
  await expect(frontToast).toHaveAttribute('data-expanded', 'true');

  await page.mouse.move(429, 0);
  await page.locator('.sonner-hitbox').dispatchEvent('pointerout', { relatedTarget: null });
  await page.waitForTimeout(350);
  await expect(frontToast).toHaveAttribute('data-expanded', 'false');
});

async function expandedLayoutHeight(page: import('@playwright/test').Page) {
  return page.evaluate((bottomPadding) => {
    const toastHeights = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
      .map((element) => Number.parseFloat(getComputedStyle(element).getPropertyValue('--initial-height')) || element.offsetHeight);
    return Math.min(760, Math.ceil(toastHeights.reduce((total, height) => total + height, 0) + (toastHeights.length - 1) * 10 + bottomPadding));
  }, toasterBottomPadding);
}

async function clearLayoutReports(page: import('@playwright/test').Page) {
  await page.evaluate(() => {
    (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.length = 0;
  });
}

async function applyNativeResizeFeedback(page: import('@playwright/test').Page, durationMs: number) {
  const appliedHeights: number[] = [];
  let reportIndex = 0;
  const deadline = Date.now() + durationMs;
  while (Date.now() < deadline) {
    const reports = await page.evaluate((index) => (
      (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports.slice(index)
    ), reportIndex);
    for (const report of reports) {
      reportIndex += 1;
      const height = Math.round(report.height);
      appliedHeights.push(height);
      await page.setViewportSize({ width: 430, height });
    }
    await page.waitForTimeout(10);
  }
  return appliedHeights;
}

async function layoutSnapshot(page: import('@playwright/test').Page) {
  return page.evaluate((bottomPadding) => {
    const bottoms = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
      .map((element) => element.getBoundingClientRect().bottom);
    const reports = (window as unknown as { __layoutReports: Array<{ height: number }> }).__layoutReports;
    return {
      visualBottom: Math.min(760, Math.ceil(Math.max(...bottoms) + bottomPadding)),
      reported: reports.at(-1)?.height ?? 0,
    };
  }, toasterBottomPadding);
}

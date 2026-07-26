import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Toaster, toast } from 'sonner';
import './styles.css';

type ToastItem = {
  id: string;
  title: string;
  message: string;
  state: 'loading' | 'success' | 'error' | 'warning' | 'info' | string;
  progress: number;
  route: string;
  primaryLabel: string;
  primaryRoute: string;
  secondaryLabel: string;
  secondaryRoute: string;
  sticky: boolean;
};

type ToastPayload = { toasts: ToastItem[]; expanded: boolean; theme?: 'light' | 'dark'; borderWidth: number };
const TOASTER_BOTTOM_PADDING = 24;
const TOAST_TRANSITION_MS = 160;
const LAYOUT_SETTLE_DELAY_MS = TOAST_TRANSITION_MS + 40;
type LayoutPhase = 'idle' | 'expanding' | 'collapsing';

declare global { interface Window { __NONSTOP_SET_TOASTS?: (payload: ToastPayload) => void } }

function invoke(cmd: string, args?: Record<string, unknown>) {
  const internals = (window as unknown as { __TAURI_INTERNALS__?: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } }).__TAURI_INTERNALS__;
  return internals?.invoke(cmd, args).catch(() => undefined);
}

function App() {
  const [theme, setTheme] = useState<'light' | 'dark'>('light');
  const [borderWidth, setBorderWidth] = useState(1);
  const heightTimer = React.useRef<number | null>(null);
  const collapseTimer = React.useRef<number | null>(null);
  const visibleToasts = React.useRef<ToastItem[]>([]);
  const expandedHeight = React.useRef(0);
  const hovering = React.useRef(false);
  const layoutPhase = React.useRef<LayoutPhase>('idle');
  const lastReportedHeight = React.useRef(0);

  const reportWindowHeight = (height: number) => {
    const targetHeight = Math.min(760, Math.ceil(height));
    if (!targetHeight || lastReportedHeight.current === targetHeight) return;
    lastReportedHeight.current = targetHeight;
    invoke('report_layout', { width: 430, height: targetHeight });
  };

  const scheduleMeasuredWindowHeight = (delayMs = 50) => {
    if (heightTimer.current) window.clearTimeout(heightTimer.current);
    heightTimer.current = window.setTimeout(() => {
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => reportWindowHeight(measureToasterHeight()));
      });
    }, delayMs);
  };

  const scheduleSettledWindowHeight = (phase: Exclude<LayoutPhase, 'idle'>) => {
    layoutPhase.current = phase;
    if (heightTimer.current) window.clearTimeout(heightTimer.current);
    heightTimer.current = window.setTimeout(() => {
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => {
          if (layoutPhase.current !== phase) return;
          if (phase === 'expanding') {
            const measuredExpandedHeight = measureExpandedToasterHeight();
            if (measuredExpandedHeight) expandedHeight.current = measuredExpandedHeight;
            reportWindowHeight(measuredExpandedHeight);
          } else {
            reportWindowHeight(measureToasterHeight());
          }
          layoutPhase.current = 'idle';
        });
      });
    }, LAYOUT_SETTLE_DELAY_MS);
  };

  const setExpanded = (next: boolean) => {
    if (hovering.current === next) {
      const toasterExpanded = document.querySelector<HTMLElement>('[data-sonner-toast][data-front="true"]')?.dataset.expanded === 'true';
      if (!next || toasterExpanded) return;
    }
    hovering.current = next;
    if (collapseTimer.current) {
      window.clearTimeout(collapseTimer.current);
      collapseTimer.current = null;
    }
    if (next) {
      const targetHeight = Math.max(expandedHeight.current, measureExpandedToasterHeight());
      if (targetHeight) expandedHeight.current = targetHeight;
      reportWindowHeight(targetHeight);
      scheduleSettledWindowHeight('expanding');
      return;
    }
    scheduleSettledWindowHeight('collapsing');
    collapseTimer.current = window.setTimeout(() => {
      resetDismissibleTimers(visibleToasts.current);
    }, 350);
  };

  useEffect(() => {
    window.__NONSTOP_SET_TOASTS = (payload) => {
      const visible = payload.toasts.slice(0, 5);
      visibleToasts.current = visible;
      setTheme(payload.theme === 'dark' ? 'dark' : 'light');
      setBorderWidth(payload.borderWidth);
      expandedHeight.current = 0;
      lastReportedHeight.current = 0;
      layoutPhase.current = 'idle';
      visible.forEach(showToast);
      scheduleMeasuredWindowHeight();
    };
    invoke('request_state');
    return () => { delete window.__NONSTOP_SET_TOASTS; };
  }, []);

  useEffect(() => {
    const syncLayout = () => {
      const measuredExpandedHeight = measureExpandedToasterHeight();
      if (measuredExpandedHeight) expandedHeight.current = measuredExpandedHeight;
      if (layoutPhase.current === 'idle') scheduleMeasuredWindowHeight();
    };
    const resizeObserver = new ResizeObserver(syncLayout);
    const observeToasts = () => {
      document.querySelectorAll<HTMLElement>('[data-sonner-toast]').forEach((element) => resizeObserver.observe(element));
    };
    const mutationObserver = new MutationObserver(() => {
      observeToasts();
      syncLayout();
    });
    mutationObserver.observe(document.body, { childList: true, subtree: true });
    observeToasts();
    return () => {
      resizeObserver.disconnect();
      mutationObserver.disconnect();
    };
  }, []);

  useEffect(() => () => {
    if (heightTimer.current) window.clearTimeout(heightTimer.current);
    if (collapseTimer.current) window.clearTimeout(collapseTimer.current);
  }, []);

  return (
    <div className="sonner-hitbox" data-theme={theme} style={{ '--notify-border-width': `${borderWidth}px` } as React.CSSProperties} onPointerOverCapture={() => setExpanded(true)} onPointerLeave={() => setExpanded(false)}>
      <Toaster theme={theme} position="top-right" visibleToasts={5} richColors gap={10} offset={0} closeButton={false} toastOptions={{ className: 'notify-sonner-toast' }} />
    </div>
  );
}

function resetDismissibleTimers(items: ToastItem[]) {
  items.filter((item) => !isManualCloseOnly(item)).forEach(showToast);
}

function ToastDescription({ item }: { item: ToastItem }) {
  const close = (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    toast.dismiss(item.id);
    invoke('close_toast', { id: item.id });
  };

  const open = (route: string) => (event: React.MouseEvent<HTMLButtonElement>) => {
    event.preventDefault();
    event.stopPropagation();
    invoke('open_route', { route });
  };

  return (
    <div className="notify-body">
      <span>{item.message || statusLabel(item.state)}</span>
      {(item.primaryRoute || item.secondaryRoute) ? (
        <div className="notify-actions">
          {item.primaryRoute ? <button className="notify-action notify-action-primary" type="button" onClick={open(item.primaryRoute)}>{item.primaryLabel || 'Open'}</button> : null}
          {item.secondaryRoute ? <button className="notify-action notify-action-secondary" type="button" onClick={open(item.secondaryRoute)}>{item.secondaryLabel || 'Details'}</button> : null}
        </div>
      ) : null}
      <button className="notify-close-button" type="button" aria-label="Close notification" onClick={close}>
        <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
          <path d="M4.25 4.25 11.75 11.75M11.75 4.25 4.25 11.75" />
        </svg>
      </button>
    </div>
  );
}

function showToast(item: ToastItem) {
  const options = {
    id: item.id,
    description: <ToastDescription item={item} />,
    duration: toastDurationMs(item),
    onAutoClose: () => invoke('close_toast', { id: item.id }),
    onDismiss: () => invoke('close_toast', { id: item.id }),
  };
  if (item.state === 'loading') toast.loading(item.title || item.id, options);
  else if (item.state === 'success') toast.success(item.title || item.id, options);
  else if (item.state === 'error') toast.error(item.title || item.id, options);
  else if (item.state === 'warning') toast.warning(item.title || item.id, options);
  else toast(item.title || item.id, options);
}

function measureExpandedToasterHeight() {
  const toastHeights = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
    .map((element) => Number.parseFloat(getComputedStyle(element).getPropertyValue('--initial-height')) || element.offsetHeight);
  return toastHeights.length
    ? Math.min(760, Math.ceil(toastHeights.reduce((total, toastHeight) => total + toastHeight, 0) + (toastHeights.length - 1) * 10 + TOASTER_BOTTOM_PADDING))
    : 0;
}

function measureToasterHeight() {
  const bottoms = [...document.querySelectorAll<HTMLElement>('[data-sonner-toast]')]
    .map((element) => {
      const toastBottom = element.getBoundingClientRect().bottom;
      if (element.dataset.front !== 'true') return toastBottom;
      const contentBottom = element.querySelector<HTMLElement>('.notify-body')?.getBoundingClientRect().bottom ?? 0;
      const intrinsicHeight = Number.parseFloat(getComputedStyle(element).getPropertyValue('--initial-height')) || element.offsetHeight;
      return Math.max(toastBottom, contentBottom, element.offsetTop + intrinsicHeight);
    });
  return bottoms.length ? Math.ceil(Math.max(...bottoms) + TOASTER_BOTTOM_PADDING) : 0;
}

function toastDurationMs(item: ToastItem) {
  if (isManualCloseOnly(item)) return Infinity;
  return 6_000;
}

function isManualCloseOnly(item: ToastItem) {
  return item.sticky || item.state === 'loading';
}

function statusLabel(state: string) {
  if (state === 'loading') return 'Running';
  if (state === 'success') return 'Complete';
  if (state === 'error') return 'Failed';
  if (state === 'warning') return 'Warning';
  return 'Information';
}

createRoot(document.getElementById('root')!).render(<App />);

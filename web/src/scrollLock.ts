/**
 * Lock document scroll without horizontal layout shift when the scrollbar
 * disappears (common when opening modals).
 */
export function lockBodyScroll(): () => void {
  const body = document.body;
  const prevOverflow = body.style.overflow;
  const prevPaddingRight = body.style.paddingRight;
  const scrollbar = window.innerWidth - document.documentElement.clientWidth;

  body.style.overflow = "hidden";
  if (scrollbar > 0) {
    const current =
      Number.parseFloat(window.getComputedStyle(body).paddingRight) || 0;
    body.style.paddingRight = `${current + scrollbar}px`;
  }

  return () => {
    body.style.overflow = prevOverflow;
    body.style.paddingRight = prevPaddingRight;
  };
}

// Vitest setup (jsdom): matchMedia + clipboard stubs for browser-mode code.
import '@testing-library/jest-dom/vitest';

if (typeof window !== 'undefined') {
  if (!window.matchMedia) {
    Object.defineProperty(window, 'matchMedia', {
      writable: true,
      value: (query: string) => ({
        matches: false,
        media: query,
        onchange: null,
        addListener: () => {},
        removeListener: () => {},
        addEventListener: () => {},
        removeEventListener: () => {},
        dispatchEvent: () => false,
      }),
    });
  }
  // jsdom does not implement Element.scrollTo (real WebViews do).
  if (!Element.prototype.scrollTo) {
    Element.prototype.scrollTo = function scrollTo() {} as typeof Element.prototype.scrollTo;
  }
  if (!navigator.clipboard) {
    Object.defineProperty(navigator, 'clipboard', {
      writable: true,
      value: {
        writeText: async () => {},
        readText: async () => '',
      },
    });
  }
}

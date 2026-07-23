export {};

function createMatchMedia(
  defaultMatches = false,
): (query: string) => MediaQueryList {
  return (query: string) => {
    const listeners = new Set<EventListener>();
    return {
      matches: defaultMatches,
      media: query,
      addEventListener: (_event: string, listener: EventListener) => {
        listeners.add(listener);
      },
      removeEventListener: (_event: string, listener: EventListener) => {
        listeners.delete(listener);
      },
      dispatchEvent: (event: Event) => {
        listeners.forEach((listener) => listener(event));
        return true;
      },
      onchange: null,
    } as unknown as MediaQueryList;
  };
}

if (typeof window !== 'undefined' && !window.matchMedia) {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: createMatchMedia(false),
  });
}

function createMockContext() {
  return {
    fillStyle: '',
    strokeStyle: '',
    lineWidth: 0,
    lineCap: '',
    lineJoin: '',
    font: '',
    textAlign: '',
    textBaseline: '',
    fillRect: () => {},
    strokeRect: () => {},
    clearRect: () => {},
    beginPath: () => {},
    closePath: () => {},
    moveTo: () => {},
    lineTo: () => {},
    stroke: () => {},
    fill: () => {},
    arc: () => {},
    save: () => {},
    restore: () => {},
    scale: () => {},
    translate: () => {},
    rotate: () => {},
    measureText: () => ({ width: 0 }),
  };
}

if (typeof window !== 'undefined' && window.HTMLCanvasElement) {
  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', {
    value: function (this: HTMLCanvasElement, contextId: string) {
      if (contextId === '2d') {
        return createMockContext();
      }
      return null;
    },
    configurable: true,
    writable: true,
  });
}

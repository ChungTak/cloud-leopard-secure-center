export {};

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

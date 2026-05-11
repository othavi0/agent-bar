import { describe, expect, it } from 'bun:test';
import { barSegments, colorForDisplay, indicatorSegments } from '../src/formatters/segments';

describe('segments helpers', () => {
  it('colorForDisplay: ok green when health >= 60', () => {
    expect(colorForDisplay(80, 'remaining')).toBe('green');
  });

  it('colorForDisplay: critical red when health < 10', () => {
    expect(colorForDisplay(5, 'remaining')).toBe('red');
  });

  it('colorForDisplay: respects used mode (display=95 -> health=5 -> red)', () => {
    expect(colorForDisplay(95, 'used')).toBe('red');
  });

  it('colorForDisplay: null -> text', () => {
    expect(colorForDisplay(null, 'remaining')).toBe('text');
  });

  it('barSegments: 20 chars total, filled proportional', () => {
    const segs = barSegments(50, 'remaining');
    const total = segs.map((s) => s.text.length).reduce((a, b) => a + b, 0);
    expect(total).toBe(20);
    expect(segs[0].text).toBe('█'.repeat(10));
  });

  it('barSegments: null -> all dimmed', () => {
    const segs = barSegments(null, 'remaining');
    expect(segs).toEqual([{ text: '░'.repeat(20), color: 'comment' }]);
  });

  it('indicatorSegments: filled dot uses health color', () => {
    expect(indicatorSegments(80, 'remaining')).toEqual([{ text: '●', color: 'green' }]);
  });

  it('indicatorSegments: null -> open dot', () => {
    expect(indicatorSegments(null, 'remaining')).toEqual([{ text: '○', color: 'comment' }]);
  });
});

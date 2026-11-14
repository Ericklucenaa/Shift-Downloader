import { describe, expect, it } from 'vitest';
import { bytes, duration, extension, fileKind, percent, percentLabel, speed } from './format';

describe('format', () => {
  it('bytes em unidades binárias com vírgula decimal', () => {
    expect(bytes(0)).toBe('0 B');
    expect(bytes(1023)).toBe('1023 B');
    expect(bytes(1536)).toBe('1,50 KB');
    expect(bytes(5 * 1024 ** 3)).toBe('5,00 GB');
    expect(bytes(150 * 1024 ** 2)).toBe('150 MB');
    expect(bytes(110 * 1024 ** 4)).toBe('110 TB');
    expect(bytes(null)).toBe('—');
    expect(bytes(1536, 'en')).toBe('1.50 KB');
  });

  it('velocidade zero vira traço, não "0 B/s"', () => {
    expect(speed(0)).toBe('—');
    expect(speed(2 * 1024 * 1024)).toBe('2,00 MB/s');
  });

  it('duração legível', () => {
    expect(duration(9)).toBe('9 s');
    expect(duration(75)).toBe('1 min 15 s');
    expect(duration(3725)).toBe('1 h 02 min');
    expect(duration(3 * 86400 + 7200)).toBe('3 d 2 h');
    expect(duration(null)).toBe('—');
  });

  it('percentual só com total conhecido', () => {
    expect(percent(50, 200)).toBe(25);
    expect(percent(50, null)).toBeNull();
    expect(percent(300, 200)).toBe(100);
    expect(percentLabel(4.25)).toBe('4,3%');
    expect(percentLabel(42.4)).toBe('42%');
  });

  it('tipo de arquivo pela extensão', () => {
    expect(extension('a.tar.GZ')).toBe('gz');
    expect(extension('.bashrc')).toBe('');
    expect(fileKind('Video_4K.MP4')).toBe('video');
    expect(fileKind('ubuntu.iso')).toBe('disk');
    expect(fileKind('setup.exe')).toBe('program');
    expect(fileKind('LEIAME')).toBe('other');
  });
});

// 히스토리 그래프의 가로축 눈금.

import { locale } from "../i18n";
import type { Tick } from "./LineChart";

const HOUR = 3_600_000;
const DAY = 24 * HOUR;

const dayLabel = new Intl.DateTimeFormat(locale, { month: "numeric", day: "numeric" });
const hourLabel = new Intl.DateTimeFormat(locale, { hour: "numeric" });

/** 시각 축: 너비에 맞게 1시간~4주 간격 중 하나를 고르고, 날짜 경계(자정)에 맞춘다. */
export function timeTicks(start: number, end: number, plotWidth: number): Tick[] {
  const maxTicks = Math.max(2, Math.floor(plotWidth / 72));
  const span = end - start;
  const steps = [HOUR, 2 * HOUR, 3 * HOUR, 6 * HOUR, 12 * HOUR, DAY, 2 * DAY, 7 * DAY, 14 * DAY, 28 * DAY];
  const step = steps.find((candidate) => span / candidate <= maxTicks) ?? 28 * DAY;

  const cursor = new Date(start);
  if (step >= DAY) {
    cursor.setHours(0, 0, 0, 0);
  } else {
    const hours = step / HOUR;
    cursor.setMinutes(0, 0, 0);
    cursor.setHours(Math.ceil(cursor.getHours() / hours) * hours);
  }
  const advance = () => {
    if (step >= DAY) cursor.setDate(cursor.getDate() + step / DAY);
    else cursor.setTime(cursor.getTime() + step);
  };
  while (cursor.getTime() < start) advance();

  const ticks: Tick[] = [];
  while (cursor.getTime() <= end && ticks.length < 60) {
    const atMidnight = cursor.getHours() === 0 && cursor.getMinutes() === 0;
    ticks.push({
      value: cursor.getTime(),
      label: step >= DAY || atMidnight ? dayLabel.format(cursor) : hourLabel.format(cursor),
    });
    advance();
  }
  return ticks;
}

/** 주기 경과 축 (0~168시간): 하루마다 */
export function cycleTicks(dayLabelFor: (day: number) => string): Tick[] {
  return Array.from({ length: 8 }, (_, day) => ({ value: day * 24, label: day === 0 ? "0" : dayLabelFor(day) }));
}

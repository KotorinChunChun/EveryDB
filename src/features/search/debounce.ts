/** 検索入力の連続変更をまとめるための、破棄可能なdebounceスケジューラ。 */
export function createSearchScheduler(run: () => void, delayMs = 150): { schedule: () => void; dispose: () => void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  return {
    schedule: () => {
      clear();
      timer = setTimeout(() => {
        timer = undefined;
        run();
      }, delayMs);
    },
    dispose: clear,
  };
}
export function formatProcessLabel(processName: string, processPid: number | null | undefined): string {
  const pid = processPid == null ? "?" : String(processPid);
  const name = processName.includes("/") ? processName.split("/").pop()! : processName;
  return `${name}(${pid})`;
}

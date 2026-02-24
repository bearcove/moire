import React from "react";
import {
  FileC,
  FileCode,
  FileCpp,
  FileJs,
  FileJsx,
  FilePy,
  FileRs,
  FileTs,
  FileTsx,
  type IconProps,
} from "@phosphor-icons/react";

const EXT_TO_ICON: Record<string, React.FC<IconProps>> = {
  rs: FileRs,
  c: FileC,
  h: FileC,
  cpp: FileCpp,
  cc: FileCpp,
  cxx: FileCpp,
  hpp: FileCpp,
  hh: FileCpp,
  py: FilePy,
  ts: FileTs,
  tsx: FileTsx,
  js: FileJs,
  jsx: FileJsx,
};

export function langIcon(sourceFile: string, size: number, className?: string): React.ReactNode {
  const ext = sourceFile.split(".").pop()?.toLowerCase() ?? "";
  const Icon = EXT_TO_ICON[ext] ?? FileCode;
  return <Icon size={size} className={className} />;
}

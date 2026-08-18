/**
 * Inline icon set. Bundled as components rather than pulled from an icon
 * package so the app ships with no runtime asset requests.
 */

type IconProps = { className?: string };

const base = "h-3.5 w-3.5 shrink-0";

export const BranchIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`${base} ${className}`}>
    <path d="M9.5 3.25a2.25 2.25 0 1 1 3 2.122V6A2.5 2.5 0 0 1 10 8.5H6a1 1 0 0 0-1 1v1.128a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.492 2.492 0 0 1 6 7h4a1 1 0 0 0 1-1v-.628A2.25 2.25 0 0 1 9.5 3.25Zm-6 0a.75.75 0 1 0 1.5 0 .75.75 0 0 0-1.5 0Zm8.25-.75a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5ZM4.25 12a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5Z" />
  </svg>
);

export const TagIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`${base} ${className}`}>
    <path d="M1 7.775V2.75C1 1.784 1.784 1 2.75 1h5.025c.464 0 .91.184 1.238.513l6.25 6.25a1.75 1.75 0 0 1 0 2.474l-5.026 5.026a1.75 1.75 0 0 1-2.474 0l-6.25-6.25A1.752 1.752 0 0 1 1 7.775Zm1.5 0c0 .066.026.13.073.177l6.25 6.25a.25.25 0 0 0 .354 0l5.025-5.025a.25.25 0 0 0 0-.354l-6.25-6.25a.25.25 0 0 0-.177-.073H2.75a.25.25 0 0 0-.25.25ZM6 5a1 1 0 1 1 0 2 1 1 0 0 1 0-2Z" />
  </svg>
);

export const CloudIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`${base} ${className}`}>
    <path d="M4.5 12a3.5 3.5 0 0 1-.5-6.965A4.501 4.501 0 0 1 12.9 5.6 3.001 3.001 0 0 1 12.5 12h-8Zm0-1.5h8a1.5 1.5 0 0 0 0-3h-.55l-.14-.53a3 3 0 0 0-5.83.36l-.11.62-.62.05a2 2 0 0 0 .25 3.5Z" />
  </svg>
);

export const FolderIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`${base} ${className}`}>
    <path d="M1.75 2h3.79c.38 0 .74.15 1.01.42L7.8 3.67h6.45c.97 0 1.75.78 1.75 1.75v7.08c0 .97-.78 1.75-1.75 1.75H1.75C.78 14.25 0 13.47 0 12.5v-8.75C0 2.78.78 2 1.75 2Zm0 1.5a.25.25 0 0 0-.25.25V12.5c0 .14.11.25.25.25h12.5a.25.25 0 0 0 .25-.25V5.42a.25.25 0 0 0-.25-.25H7.49a.75.75 0 0 1-.53-.22L5.71 3.7a.25.25 0 0 0-.17-.07Z" />
  </svg>
);

export const RefreshIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`${base} ${className}`}>
    <path d="M8 2.5a5.5 5.5 0 0 0-5.06 3.35.75.75 0 1 1-1.38-.59A7 7 0 0 1 13.5 4.6V3.25a.75.75 0 0 1 1.5 0v3.5a.75.75 0 0 1-.75.75h-3.5a.75.75 0 0 1 0-1.5h1.86A5.49 5.49 0 0 0 8 2.5Zm-6.25 6a.75.75 0 0 1 .75.75v.15A5.5 5.5 0 0 0 13.06 10.15a.75.75 0 1 1 1.38.59A7 7 0 0 1 2.5 11.4v1.35a.75.75 0 0 1-1.5 0v-3.5a.75.75 0 0 1 .75-.75Z" />
  </svg>
);

export const ChevronIcon = ({ className = "" }: IconProps) => (
  <svg viewBox="0 0 16 16" fill="currentColor" className={`h-3 w-3 shrink-0 ${className}`}>
    <path d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.75.75 0 0 1-1.06-1.06L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z" />
  </svg>
);

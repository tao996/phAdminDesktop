import type { ReactNode } from "react";

export type IconName =
  | "server"
  | "folder"
  | "plus"
  | "search"
  | "chevron-down"
  | "chevron-right"
  | "terminal"
  | "play"
  | "rocket"
  | "refresh"
  | "more"
  | "close"
  | "settings"
  | "command"
  | "activity"
  | "database"
  | "globe"
  | "box"
  | "file-text"
  | "check"
  | "alert"
  | "clock"
  | "arrow-up-right"
  | "copy"
  | "download"
  | "shield"
  | "git-branch"
  | "layers"
  | "panel-bottom"
  | "trash"
  | "zap"
  | "sliders"
  | "wifi"
  | "wifi-off"
  | "eye"
  | "eye-off";

interface IconProps {
  name: IconName;
  size?: number;
  className?: string;
}

function paths(name: IconName): ReactNode {
  switch (name) {
    case "server":
      return (
        <>
          <rect x="3" y="4" width="18" height="7" rx="2" />
          <rect x="3" y="13" width="18" height="7" rx="2" />
          <path d="M7 7.5h.01M7 16.5h.01" />
        </>
      );
    case "folder":
      return <path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2Z" />;
    case "plus":
      return <path d="M12 5v14M5 12h14" />;
    case "search":
      return (
        <>
          <circle cx="11" cy="11" r="7" />
          <path d="m20 20-3.5-3.5" />
        </>
      );
    case "chevron-down":
      return <path d="m6 9 6 6 6-6" />;
    case "chevron-right":
      return <path d="m9 18 6-6-6-6" />;
    case "terminal":
      return <path d="m4 17 6-6-6-6M12 19h8" />;
    case "play":
      return <path d="m7 4 13 8-13 8Z" />;
    case "rocket":
      return (
        <>
          <path d="M4.5 16.5c-1.5 1.5-2 5-2 5s3.5-.5 5-2" />
          <path d="m12 15 3 3" />
          <path d="M9 12 6 9" />
          <path d="M14.5 4.5c2-2 5.5-2 5.5-2s0 3.5-2 5.5L12 14l-2-2Z" />
        </>
      );
    case "refresh":
      return (
        <>
          <path d="M20 11a8 8 0 1 0 2 5.3" />
          <path d="M20 4v7h-7" />
        </>
      );
    case "more":
      return (
        <>
          <circle cx="5" cy="12" r="1" />
          <circle cx="12" cy="12" r="1" />
          <circle cx="19" cy="12" r="1" />
        </>
      );
    case "close":
      return <path d="M18 6 6 18M6 6l12 12" />;
    case "settings":
      return (
        <>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z" />
        </>
      );
    case "command":
      return (
        <path d="M9 6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3Z" />
      );
    case "activity":
      return <path d="M3 12h4l2.5-6 4 12 2.5-6h5" />;
    case "database":
      return (
        <>
          <ellipse cx="12" cy="5" rx="8" ry="3" />
          <path d="M4 5v7c0 1.7 3.6 3 8 3s8-1.3 8-3V5" />
          <path d="M4 12v7c0 1.7 3.6 3 8 3s8-1.3 8-3v-7" />
        </>
      );
    case "globe":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M3 12h18M12 3c2.5 2.5 3.8 5.6 3.8 9S14.5 18.5 12 21c-2.5-2.5-3.8-5.6-3.8-9S9.5 5.5 12 3Z" />
        </>
      );
    case "box":
      return (
        <>
          <path d="m12 3 8 4.5v9L12 21l-8-4.5v-9Z" />
          <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
        </>
      );
    case "file-text":
      return (
        <>
          <path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9Z" />
          <path d="M14 3v6h6M8 13h8M8 17h5" />
        </>
      );
    case "check":
      return <path d="m5 12 4 4L19 6" />;
    case "alert":
      return (
        <>
          <path d="m12 3 9 16H3Z" />
          <path d="M12 9v4M12 17h.01" />
        </>
      );
    case "clock":
      return (
        <>
          <circle cx="12" cy="12" r="9" />
          <path d="M12 7v5l3 2" />
        </>
      );
    case "arrow-up-right":
      return <path d="M7 17 17 7M7 7h10v10" />;
    case "copy":
      return (
        <>
          <rect x="9" y="9" width="12" height="12" rx="2" />
          <path d="M5 15H3a2 2 0 0 1-2-2V3a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v2" />
        </>
      );
    case "download":
      return (
        <>
          <path d="M12 3v12m0 0 4-4m-4 4-4-4" />
          <path d="M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
        </>
      );
    case "shield":
      return <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10Z" />;
    case "git-branch":
      return (
        <>
          <circle cx="6" cy="5" r="2" />
          <circle cx="18" cy="6" r="2" />
          <circle cx="6" cy="19" r="2" />
          <path d="M6 7v10M8 7c0 5 4 6 8 6" />
        </>
      );
    case "layers":
      return (
        <>
          <path d="m12 2 9 5-9 5-9-5Z" />
          <path d="m3 12 9 5 9-5M3 17l9 5 9-5" />
        </>
      );
    case "panel-bottom":
      return (
        <>
          <rect x="3" y="3" width="18" height="18" rx="2" />
          <path d="M3 15h18" />
        </>
      );
    case "trash":
      return (
        <>
          <path d="M3 6h18M8 6V4h8v2M19 6l-1 15H6L5 6" />
          <path d="M10 11v6M14 11v6" />
        </>
      );
    case "zap":
      return <path d="M13 2 3 14h8l-1 8 10-12h-8Z" />;
    case "sliders":
      return (
        <>
          <path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3" />
          <path d="M1 14h6M9 8h6M17 16h6" />
        </>
      );
    case "wifi":
      return (
        <>
          <path d="M5 12.5a10 10 0 0 1 14 0M8.5 16a5 5 0 0 1 7 0M12 19.5h.01" />
        </>
      );
    case "wifi-off":
      return (
        <>
          <path d="m2 2 20 20M8.5 16a5 5 0 0 1 7 0M5 12.5a10 10 0 0 1 4-2.3M15 10.2a10 10 0 0 1 4 2.3M12 19.5h.01" />
        </>
      );
    case "eye":
      return (
        <>
          <path d="M2 12s3.5-6 10-6 10 6 10 6-3.5 6-10 6-10-6-10-6Z" />
          <circle cx="12" cy="12" r="3" />
        </>
      );
    case "eye-off":
      return (
        <>
          <path d="m2 2 20 20" />
          <path d="M6.7 6.7C3.7 8.6 2 12 2 12s3.5 6 10 6c1.7 0 3.2-.5 4.5-1.2M9.9 5.2A11 11 0 0 1 12 5c6.5 0 10 7 10 7a18 18 0 0 1-2.2 3.2" />
          <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
        </>
      );
  }
}

export function Icon({ name, size = 16, className }: IconProps) {
  return (
    <svg
      className={className}
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths(name)}
    </svg>
  );
}

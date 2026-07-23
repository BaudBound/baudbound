import type { CSSProperties } from "react";
import { Toaster as Sonner, type ToasterProps } from "sonner";

export function Toaster(props: ToasterProps) {
  return (
    <Sonner
      className="toaster group"
      icons={{
        error: null,
        info: null,
        loading: null,
        success: null,
        warning: null,
      }}
      style={
        {
          "--border-radius": "var(--radius-md)",
          "--error-bg": "#170b0e",
          "--error-border": "rgb(230 45 62 / 0.45)",
          "--error-text": "#ffd7dc",
          "--normal-bg": "#0d1017",
          "--normal-border": "#242a3a",
          "--normal-text": "#d8deec",
          "--success-bg": "#081911",
          "--success-border": "rgb(62 207 142 / 0.45)",
          "--success-text": "#c8ffe6",
          "--warning-bg": "#1b1306",
          "--warning-border": "rgb(245 166 35 / 0.45)",
          "--warning-text": "#ffe4b0",
        } as CSSProperties
      }
      theme="dark"
      toastOptions={{
        classNames: {
          toast: "cn-toast",
        },
      }}
      {...props}
    />
  );
}

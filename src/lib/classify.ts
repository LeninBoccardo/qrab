// Frontend display helpers for QrKind. Pure functions; no Tauri imports.

import type { Component } from "solid-js";
import {
  FileText,
  HelpCircle,
  Link,
  Mail,
  Phone,
  User,
  Wifi,
} from "lucide-solid";
import type { QrKind } from "./types";

export function kindLabel(kind: QrKind): string {
  switch (kind) {
    case "url":
      return "Link";
    case "text":
      return "Text";
    case "wifi":
      return "Wi-Fi";
    case "vcard":
      return "Contact";
    case "email":
      return "Email";
    case "phone":
      return "Phone";
    case "other":
      return "Other";
  }
}

type IconProps = { size?: number; class?: string };

export function kindIcon(kind: QrKind): Component<IconProps> {
  switch (kind) {
    case "url":
      return Link;
    case "text":
      return FileText;
    case "wifi":
      return Wifi;
    case "vcard":
      return User;
    case "email":
      return Mail;
    case "phone":
      return Phone;
    case "other":
      return HelpCircle;
  }
}

/** Does this row have a primary "Open" action? URLs only. */
export function isOpenable(kind: QrKind): boolean {
  return kind === "url";
}

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import zh from "./locales/zh.json";

export type Language = "zh" | "en";

const DEFAULT_LANGUAGE: Language = "zh";

const getInitialLanguage = (): Language => {
  if (typeof window !== "undefined") {
    try {
      const stored = window.localStorage.getItem("language");
      if (stored === "zh" || stored === "en") {
        return stored;
      }
    } catch {
      // ignore
    }
  }

  const navLang = navigator.language?.toLowerCase() ?? "";
  if (navLang.startsWith("zh")) return "zh";
  return "en";
};

const resources = {
  en: { translation: en },
  zh: { translation: zh },
};

const syncDocumentLanguage = (lang: Language) => {
  if (typeof document !== "undefined") {
    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
  }
};

i18n.use(initReactI18next).init({
  resources,
  lng: getInitialLanguage(),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
  debug: false,
});

syncDocumentLanguage(getCurrentLanguage());

export function setLanguage(lang: Language) {
  i18n.changeLanguage(lang);
  syncDocumentLanguage(lang);
  try {
    localStorage.setItem("language", lang);
  } catch {
    // ignore
  }
}

export function getCurrentLanguage(): Language {
  return (i18n.language?.startsWith("zh") ? "zh" : "en") as Language;
}

export default i18n;

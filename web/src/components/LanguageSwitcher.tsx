import { useState, useEffect } from "react";
import { setLanguage, getCurrentLanguage } from "@/i18n";
type Lang = ReturnType<typeof getCurrentLanguage>;

export function LanguageSwitcher() {
  const [lang, setLang] = useState<Lang>("zh");

  useEffect(() => {
    setLang(getCurrentLanguage());
  }, []);

  const toggle = () => {
    const next: Lang = lang === "zh" ? "en" : "zh";
    setLang(next);
    setLanguage(next);
  };

  return (
    <button
      type="button"
      onClick={toggle}
      title={lang === "zh" ? "Switch to English" : "切换到中文"}
      aria-label={lang === "zh" ? "Switch to English" : "切换到中文"}
    >
      {lang === "zh" ? "EN" : "中"}
    </button>
  );
}

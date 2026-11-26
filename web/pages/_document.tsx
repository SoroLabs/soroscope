import { Head, Html, Main, NextScript } from "next/document";

const themeInitializer = `
(function () {
  try {
    var storageKey = "theme";
    var storedTheme = localStorage.getItem(storageKey);
    var validThemes = ["light", "dark", "system"];
    var theme = validThemes.indexOf(storedTheme) !== -1
      ? storedTheme
      : "dark";

    if (theme === "system") {
      theme = window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    }

    var root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add(theme);
    root.style.colorScheme = theme;
  } catch (error) {
    // Keep the default dark theme if storage or browser APIs are unavailable.
    document.documentElement.classList.remove("light");
    document.documentElement.classList.add("dark");
    document.documentElement.style.colorScheme = "dark";
  }
})();
`;

export default function Document() {
  return (
    <Html lang="en">
      <Head>
        <meta name="color-scheme" content="dark light" />
        <script dangerouslySetInnerHTML={{ __html: themeInitializer }} />
      </Head>
      <body>
        <Main />
        <NextScript />
      </body>
    </Html>
  );
}
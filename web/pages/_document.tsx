import { Head, Html, Main, NextScript } from "next/document";

const themeInitializer = `
(function() {
  try {
    var storageKey = "theme";
    var defaultTheme = "dark";
    var theme = localStorage.getItem(storageKey) || defaultTheme;

    if (theme === "system") {
      theme = window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
    }

    var root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add(theme);
    root.style.colorScheme = theme;
  } catch (_) {}
})();
`;

export default function Document() {
  return (
    <Html>
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

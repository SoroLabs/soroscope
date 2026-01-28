module.exports = [
"[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)", ((__turbopack_context__, module, exports) => {

const mod = __turbopack_context__.x("react/jsx-dev-runtime", () => require("react/jsx-dev-runtime"));

module.exports = mod;
}),
"[project]/context/WalletContext.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "WalletProvider",
    ()=>WalletProvider,
    "useWallet",
    ()=>useWallet
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
var __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react [external] (react, cjs)");
"use client";
;
;
const WalletContext = /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["createContext"])(undefined);
const useWallet = ()=>{
    const context = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useContext"])(WalletContext);
    if (!context) {
        throw new Error("useWallet must be used within a WalletProvider");
    }
    return context;
};
const WalletProvider = ({ children })=>{
    const [address, setAddress] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(null);
    const [isConnecting, setIsConnecting] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(false);
    const [selectedWalletId, setSelectedWalletId] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(null);
    const [isModalOpen, setIsModalOpen] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(false);
    const [error, setError] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(null);
    const [kit, setKit] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(null);
    (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useEffect"])(()=>{
        const initKit = async ()=>{
            try {
                const walletKitModule = await __turbopack_context__.A("[project]/node_modules/@creit.tech/stellar-wallets-kit/index.mjs [ssr] (ecmascript, async loader)");
                const kitInstance = new walletKitModule.StellarWalletsKit({
                    network: walletKitModule.WalletNetwork.TESTNET,
                    selectedWalletId: walletKitModule.FREIGHTER_ID,
                    modules: walletKitModule.allowAllModules()
                });
                setKit(kitInstance);
                const savedAddress = localStorage.getItem("inheritx_wallet_address");
                const savedWalletId = localStorage.getItem("inheritx_wallet_id");
                if (savedAddress && savedWalletId) {
                    setAddress(savedAddress);
                    setSelectedWalletId(savedWalletId);
                }
            } catch (err) {
                console.error("Failed to initialize wallet kit:", err);
                setError("Failed to load wallet kit");
            }
        };
        initKit();
    }, []);
    const supportedWallets = [
        {
            id: "freighter",
            name: "Freighter",
            icon: "/icons/freighter.png"
        },
        {
            id: "albedo",
            name: "Albedo",
            icon: "/icons/albedo.png"
        },
        {
            id: "xbull",
            name: "xBull",
            icon: "/icons/xbull.png"
        },
        {
            id: "rabet",
            name: "Rabet",
            icon: "/icons/rabet.png"
        },
        {
            id: "lobstr",
            name: "Lobstr",
            icon: "/icons/lobstr.png"
        }
    ];
    const connectWallet = async (moduleId)=>{
        if (!kit) {
            setError("Wallet kit not loaded yet");
            return;
        }
        setIsConnecting(true);
        setError(null);
        try {
            kit.setWallet(moduleId);
            const { address: walletAddress } = await kit.getAddress();
            setAddress(walletAddress);
            setSelectedWalletId(moduleId);
            localStorage.setItem("inheritx_wallet_address", walletAddress);
            localStorage.setItem("inheritx_wallet_id", moduleId);
            setIsModalOpen(false);
        } catch (err) {
            const errorMessage = err?.message || "Connection failed";
            setError(errorMessage);
            console.error("Wallet connection failed:", err);
        } finally{
            setIsConnecting(false);
        }
    };
    const disconnect = async ()=>{
        if (kit) {
            try {
                await kit.disconnect();
            } catch (err) {
                console.error("Disconnect error:", err);
            }
        }
        setAddress(null);
        setSelectedWalletId(null);
        setError(null);
        localStorage.removeItem("inheritx_wallet_address");
        localStorage.removeItem("inheritx_wallet_id");
    };
    const openModal = ()=>{
        setError(null);
        setIsModalOpen(true);
    };
    const closeModal = ()=>{
        setError(null);
        setIsModalOpen(false);
    };
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(WalletContext.Provider, {
        value: {
            connect: connectWallet,
            disconnect,
            address,
            isConnected: !!address,
            isConnecting,
            selectedWalletId,
            openModal,
            closeModal,
            isModalOpen,
            supportedWallets,
            error
        },
        children: children
    }, void 0, false, {
        fileName: "[project]/context/WalletContext.tsx",
        lineNumber: 126,
        columnNumber: 5
    }, ("TURBOPACK compile-time value", void 0));
};
}),
"[project]/pages/_app.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "default",
    ()=>App
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
var __TURBOPACK__imported__module__$5b$project$5d2f$context$2f$WalletContext$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/context/WalletContext.tsx [ssr] (ecmascript)");
;
;
;
function App({ Component, pageProps }) {
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(__TURBOPACK__imported__module__$5b$project$5d2f$context$2f$WalletContext$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__["WalletProvider"], {
        children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(Component, {
            ...pageProps
        }, void 0, false, {
            fileName: "[project]/pages/_app.tsx",
            lineNumber: 8,
            columnNumber: 7
        }, this)
    }, void 0, false, {
        fileName: "[project]/pages/_app.tsx",
        lineNumber: 7,
        columnNumber: 5
    }, this);
}
}),
];

//# sourceMappingURL=%5Broot-of-the-server%5D__df418542._.js.map
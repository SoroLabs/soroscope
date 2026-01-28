module.exports = [
"[project]/components/DynamicForm.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "DynamicForm",
    ()=>DynamicForm
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
var __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react [external] (react, cjs)");
'use client';
;
;
function DynamicForm({ func, onSubmit, loading }) {
    const [formData, setFormData] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])({});
    const handleChange = (name, value)=>{
        setFormData((prev)=>({
                ...prev,
                [name]: value
            }));
    };
    const handleSubmit = (e)=>{
        e.preventDefault();
        onSubmit(formData);
    };
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("form", {
        onSubmit: handleSubmit,
        style: {
            display: 'flex',
            flexDirection: 'column',
            gap: '16px'
        },
        children: [
            func.inputs.length === 0 ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                style: {
                    color: '#8b949e',
                    fontSize: '14px'
                },
                children: "No inputs required"
            }, void 0, false, {
                fileName: "[project]/components/DynamicForm.tsx",
                lineNumber: 29,
                columnNumber: 9
            }, this) : func.inputs.map((input)=>/*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                    style: {
                        display: 'flex',
                        flexDirection: 'column',
                        gap: '4px'
                    },
                    children: [
                        /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("label", {
                            style: {
                                fontSize: '14px',
                                fontWeight: '500',
                                color: '#c9d1d9'
                            },
                            children: [
                                input.name,
                                input.optional ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                    style: {
                                        color: '#8b949e',
                                        marginLeft: '4px'
                                    },
                                    children: "(optional)"
                                }, void 0, false, {
                                    fileName: "[project]/components/DynamicForm.tsx",
                                    lineNumber: 49,
                                    columnNumber: 17
                                }, this) : /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                    style: {
                                        color: '#fb8500'
                                    },
                                    children: "*"
                                }, void 0, false, {
                                    fileName: "[project]/components/DynamicForm.tsx",
                                    lineNumber: 51,
                                    columnNumber: 17
                                }, this)
                            ]
                        }, void 0, true, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 40,
                            columnNumber: 13
                        }, this),
                        input.description && /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                            style: {
                                fontSize: '12px',
                                color: '#8b949e',
                                margin: '0'
                            },
                            children: input.description
                        }, void 0, false, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 55,
                            columnNumber: 15
                        }, this),
                        input.type === 'address' ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("input", {
                            type: "text",
                            placeholder: "Enter Stellar address (G...)",
                            value: formData[input.name] || '',
                            onChange: (e)=>handleChange(input.name, e.target.value),
                            required: !input.optional,
                            style: {
                                padding: '8px 12px',
                                border: '1px solid #30363d',
                                borderRadius: '6px',
                                fontSize: '14px',
                                fontFamily: 'monospace',
                                boxSizing: 'border-box',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9'
                            }
                        }, void 0, false, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 66,
                            columnNumber: 15
                        }, this) : input.type === 'u32' || input.type === 'u128' || input.type === 'i128' ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("input", {
                            type: "number",
                            placeholder: `Enter ${input.type} value`,
                            value: formData[input.name] || '',
                            onChange: (e)=>handleChange(input.name, e.target.value),
                            required: !input.optional,
                            style: {
                                padding: '8px 12px',
                                border: '1px solid #30363d',
                                borderRadius: '6px',
                                fontSize: '14px',
                                boxSizing: 'border-box',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9'
                            }
                        }, void 0, false, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 84,
                            columnNumber: 15
                        }, this) : input.type === 'string' || input.type === 'symbol' ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("input", {
                            type: "text",
                            placeholder: `Enter ${input.type}`,
                            value: formData[input.name] || '',
                            onChange: (e)=>handleChange(input.name, e.target.value),
                            required: !input.optional,
                            style: {
                                padding: '8px 12px',
                                border: '1px solid #30363d',
                                borderRadius: '6px',
                                fontSize: '14px',
                                boxSizing: 'border-box',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9'
                            }
                        }, void 0, false, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 101,
                            columnNumber: 15
                        }, this) : input.type === 'bool' ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("select", {
                            value: formData[input.name] === undefined ? '' : formData[input.name],
                            onChange: (e)=>handleChange(input.name, e.target.value === 'true'),
                            required: !input.optional,
                            style: {
                                padding: '8px 12px',
                                border: '1px solid #30363d',
                                borderRadius: '6px',
                                fontSize: '14px',
                                boxSizing: 'border-box',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9'
                            },
                            children: [
                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("option", {
                                    value: "",
                                    children: "Select value"
                                }, void 0, false, {
                                    fileName: "[project]/components/DynamicForm.tsx",
                                    lineNumber: 132,
                                    columnNumber: 17
                                }, this),
                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("option", {
                                    value: "true",
                                    children: "True"
                                }, void 0, false, {
                                    fileName: "[project]/components/DynamicForm.tsx",
                                    lineNumber: 133,
                                    columnNumber: 17
                                }, this),
                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("option", {
                                    value: "false",
                                    children: "False"
                                }, void 0, false, {
                                    fileName: "[project]/components/DynamicForm.tsx",
                                    lineNumber: 134,
                                    columnNumber: 17
                                }, this)
                            ]
                        }, void 0, true, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 118,
                            columnNumber: 15
                        }, this) : /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("input", {
                            type: "text",
                            placeholder: "Enter value",
                            value: formData[input.name] || '',
                            onChange: (e)=>handleChange(input.name, e.target.value),
                            required: !input.optional,
                            style: {
                                padding: '8px 12px',
                                border: '1px solid #30363d',
                                borderRadius: '6px',
                                fontSize: '14px',
                                boxSizing: 'border-box',
                                backgroundColor: '#0d1117',
                                color: '#c9d1d9'
                            }
                        }, void 0, false, {
                            fileName: "[project]/components/DynamicForm.tsx",
                            lineNumber: 137,
                            columnNumber: 15
                        }, this)
                    ]
                }, input.name, true, {
                    fileName: "[project]/components/DynamicForm.tsx",
                    lineNumber: 32,
                    columnNumber: 11
                }, this)),
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    display: 'flex',
                    gap: '12px',
                    marginTop: '8px'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                        type: "submit",
                        disabled: loading,
                        style: {
                            padding: '10px 20px',
                            backgroundColor: loading ? '#30363d' : '#00d9ff',
                            color: loading ? '#8b949e' : '#0f1117',
                            border: 'none',
                            borderRadius: '6px',
                            fontSize: '14px',
                            fontWeight: '600',
                            cursor: loading ? 'not-allowed' : 'pointer',
                            flex: 1
                        },
                        children: loading ? 'Simulating...' : 'Simulate'
                    }, void 0, false, {
                        fileName: "[project]/components/DynamicForm.tsx",
                        lineNumber: 158,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                        type: "button",
                        disabled: loading,
                        style: {
                            padding: '10px 20px',
                            backgroundColor: loading ? '#30363d' : '#a371f7',
                            color: loading ? '#8b949e' : '#fff',
                            border: 'none',
                            borderRadius: '6px',
                            fontSize: '14px',
                            fontWeight: '600',
                            cursor: loading ? 'not-allowed' : 'pointer',
                            flex: 1
                        },
                        children: loading ? 'Invoking...' : 'Live (Invoke)'
                    }, void 0, false, {
                        fileName: "[project]/components/DynamicForm.tsx",
                        lineNumber: 175,
                        columnNumber: 9
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/components/DynamicForm.tsx",
                lineNumber: 157,
                columnNumber: 7
            }, this)
        ]
    }, void 0, true, {
        fileName: "[project]/components/DynamicForm.tsx",
        lineNumber: 27,
        columnNumber: 5
    }, this);
}
}),
"[project]/components/Resultviewer.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "ResultViewer",
    ()=>ResultViewer
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
'use client';
;
function ResultViewer({ result }) {
    if (!result) {
        return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
            style: {
                padding: '24px',
                backgroundColor: '#0d1117',
                borderRadius: '8px',
                textAlign: 'center',
                color: '#8b949e',
                border: '1px solid #30363d'
            },
            children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                children: "No results yet. Execute a contract function to see results here."
            }, void 0, false, {
                fileName: "[project]/components/Resultviewer.tsx",
                lineNumber: 22,
                columnNumber: 9
            }, this)
        }, void 0, false, {
            fileName: "[project]/components/Resultviewer.tsx",
            lineNumber: 12,
            columnNumber: 7
        }, this);
    }
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
        style: {
            padding: '24px',
            backgroundColor: '#0d1117',
            borderRadius: '8px',
            borderLeft: `4px solid ${result.success ? '#00d9ff' : '#fb8500'}`,
            border: `1px solid #30363d`
        },
        children: [
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    marginBottom: '16px'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h3", {
                        style: {
                            margin: '0 0 8px 0',
                            color: result.success ? '#00d9ff' : '#fb8500',
                            fontSize: '16px',
                            fontWeight: '600'
                        },
                        children: result.success ? '✓ Success' : '✗ Error'
                    }, void 0, false, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 38,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                        style: {
                            margin: '0',
                            color: '#8b949e',
                            fontSize: '13px'
                        },
                        children: new Date(result.timestamp).toLocaleString()
                    }, void 0, false, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 48,
                        columnNumber: 9
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/components/Resultviewer.tsx",
                lineNumber: 37,
                columnNumber: 7
            }, this),
            result.error ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    backgroundColor: '#0d1117',
                    padding: '12px',
                    borderRadius: '6px',
                    marginBottom: '12px',
                    fontSize: '13px',
                    color: '#fb8500',
                    fontFamily: 'monospace',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-all',
                    border: '1px solid #30363d'
                },
                children: result.error
            }, void 0, false, {
                fileName: "[project]/components/Resultviewer.tsx",
                lineNumber: 54,
                columnNumber: 9
            }, this) : result.result && /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    backgroundColor: '#0d1117',
                    padding: '12px',
                    borderRadius: '6px',
                    marginBottom: '12px',
                    fontSize: '13px',
                    fontFamily: 'monospace',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-all',
                    color: '#58a6ff',
                    border: '1px solid #30363d'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("strong", {
                        children: "Result:"
                    }, void 0, false, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 86,
                        columnNumber: 13
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("br", {}, void 0, false, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 87,
                        columnNumber: 13
                    }, this),
                    JSON.stringify(result.result, null, 2)
                ]
            }, void 0, true, {
                fileName: "[project]/components/Resultviewer.tsx",
                lineNumber: 72,
                columnNumber: 11
            }, this),
            result.resourceCost && /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gap: '12px',
                    fontSize: '13px'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            backgroundColor: '#0d1117',
                            padding: '12px',
                            borderRadius: '6px',
                            border: '1px solid #30363d'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("strong", {
                                style: {
                                    color: '#00d9ff'
                                },
                                children: "Fee:"
                            }, void 0, false, {
                                fileName: "[project]/components/Resultviewer.tsx",
                                lineNumber: 110,
                                columnNumber: 13
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                style: {
                                    margin: '4px 0 0 0',
                                    color: '#8b949e'
                                },
                                children: [
                                    result.resourceCost.fee,
                                    " XLM"
                                ]
                            }, void 0, true, {
                                fileName: "[project]/components/Resultviewer.tsx",
                                lineNumber: 111,
                                columnNumber: 13
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 102,
                        columnNumber: 11
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            backgroundColor: '#0d1117',
                            padding: '12px',
                            borderRadius: '6px',
                            border: '1px solid #30363d'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("strong", {
                                style: {
                                    color: '#00d9ff'
                                },
                                children: "Instructions:"
                            }, void 0, false, {
                                fileName: "[project]/components/Resultviewer.tsx",
                                lineNumber: 121,
                                columnNumber: 13
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                style: {
                                    margin: '4px 0 0 0',
                                    color: '#8b949e'
                                },
                                children: result.resourceCost.instructions
                            }, void 0, false, {
                                fileName: "[project]/components/Resultviewer.tsx",
                                lineNumber: 122,
                                columnNumber: 13
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/components/Resultviewer.tsx",
                        lineNumber: 113,
                        columnNumber: 11
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/components/Resultviewer.tsx",
                lineNumber: 94,
                columnNumber: 9
            }, this)
        ]
    }, void 0, true, {
        fileName: "[project]/components/Resultviewer.tsx",
        lineNumber: 28,
        columnNumber: 5
    }, this);
}
}),
"[project]/components/InnovocationHistory.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "InvocationHistory",
    ()=>InvocationHistory,
    "useInvocationHistory",
    ()=>useInvocationHistory
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
var __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react [external] (react, cjs)");
'use client';
;
;
const HISTORY_KEY = 'soroban-invocation-history';
const MAX_HISTORY = 10;
function useInvocationHistory() {
    const [history, setHistory] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])([]);
    const [mounted, setMounted] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(false);
    (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useEffect"])(()=>{
        setMounted(true);
        const saved = localStorage.getItem(HISTORY_KEY);
        if (saved) {
            try {
                setHistory(JSON.parse(saved));
            } catch  {
                setHistory([]);
            }
        }
    }, []);
    const addToHistory = (result)=>{
        setHistory((prev)=>{
            const updated = [
                result,
                ...prev
            ].slice(0, MAX_HISTORY);
            if (mounted) {
                localStorage.setItem(HISTORY_KEY, JSON.stringify(updated));
            }
            return updated;
        });
    };
    const clearHistory = ()=>{
        setHistory([]);
        if (mounted) {
            localStorage.removeItem(HISTORY_KEY);
        }
    };
    return {
        history,
        addToHistory,
        clearHistory,
        mounted
    };
}
function InvocationHistory({ onSelectResult }) {
    const { history, clearHistory } = useInvocationHistory();
    if (history.length === 0) {
        return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
            style: {
                padding: '24px',
                backgroundColor: '#0d1117',
                borderRadius: '8px',
                textAlign: 'center',
                color: '#8b949e',
                border: '1px solid #30363d'
            },
            children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                children: "No invocation history yet."
            }, void 0, false, {
                fileName: "[project]/components/InnovocationHistory.tsx",
                lineNumber: 64,
                columnNumber: 9
            }, this)
        }, void 0, false, {
            fileName: "[project]/components/InnovocationHistory.tsx",
            lineNumber: 54,
            columnNumber: 7
        }, this);
    }
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
        children: [
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    display: 'flex',
                    justifyContent: 'space-between',
                    alignItems: 'center',
                    marginBottom: '12px'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h3", {
                        style: {
                            margin: '0',
                            fontSize: '16px',
                            fontWeight: '600',
                            color: '#c9d1d9'
                        },
                        children: "Recent Invocations"
                    }, void 0, false, {
                        fileName: "[project]/components/InnovocationHistory.tsx",
                        lineNumber: 79,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                        onClick: clearHistory,
                        style: {
                            padding: '6px 12px',
                            backgroundColor: '#161b22',
                            border: '1px solid #30363d',
                            borderRadius: '6px',
                            fontSize: '12px',
                            cursor: 'pointer',
                            color: '#8b949e'
                        },
                        children: "Clear History"
                    }, void 0, false, {
                        fileName: "[project]/components/InnovocationHistory.tsx",
                        lineNumber: 82,
                        columnNumber: 9
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/components/InnovocationHistory.tsx",
                lineNumber: 71,
                columnNumber: 7
            }, this),
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                style: {
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '8px'
                },
                children: history.map((item)=>/*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                        onClick: ()=>onSelectResult(item),
                        style: {
                            padding: '12px',
                            backgroundColor: '#0d1117',
                            border: '1px solid #30363d',
                            borderRadius: '6px',
                            textAlign: 'left',
                            cursor: 'pointer',
                            transition: 'all 0.2s'
                        },
                        onMouseEnter: (e)=>{
                            e.currentTarget.style.backgroundColor = '#161b22';
                            e.currentTarget.style.borderColor = '#00d9ff';
                        },
                        onMouseLeave: (e)=>{
                            e.currentTarget.style.backgroundColor = '#0d1117';
                            e.currentTarget.style.borderColor = '#30363d';
                        },
                        children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                            style: {
                                display: 'flex',
                                justifyContent: 'space-between',
                                alignItems: 'center'
                            },
                            children: [
                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                    children: [
                                        /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                            style: {
                                                margin: '0 0 4px 0',
                                                fontSize: '14px',
                                                fontWeight: '500',
                                                color: '#c9d1d9'
                                            },
                                            children: [
                                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                                    style: {
                                                        color: item.success ? '#00d9ff' : '#fb8500',
                                                        marginRight: '8px'
                                                    },
                                                    children: item.success ? '✓' : '✗'
                                                }, void 0, false, {
                                                    fileName: "[project]/components/InnovocationHistory.tsx",
                                                    lineNumber: 130,
                                                    columnNumber: 19
                                                }, this),
                                                item.functionName
                                            ]
                                        }, void 0, true, {
                                            fileName: "[project]/components/InnovocationHistory.tsx",
                                            lineNumber: 129,
                                            columnNumber: 17
                                        }, this),
                                        /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                            style: {
                                                margin: '0',
                                                fontSize: '12px',
                                                color: '#8b949e'
                                            },
                                            children: new Date(item.timestamp).toLocaleTimeString()
                                        }, void 0, false, {
                                            fileName: "[project]/components/InnovocationHistory.tsx",
                                            lineNumber: 140,
                                            columnNumber: 17
                                        }, this)
                                    ]
                                }, void 0, true, {
                                    fileName: "[project]/components/InnovocationHistory.tsx",
                                    lineNumber: 128,
                                    columnNumber: 15
                                }, this),
                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                    style: {
                                        textAlign: 'right',
                                        fontSize: '12px',
                                        color: '#8b949e'
                                    },
                                    children: item.error ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                        style: {
                                            color: '#fb8500'
                                        },
                                        children: "Error"
                                    }, void 0, false, {
                                        fileName: "[project]/components/InnovocationHistory.tsx",
                                        lineNumber: 146,
                                        columnNumber: 19
                                    }, this) : /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                        style: {
                                            color: '#00d9ff'
                                        },
                                        children: "Success"
                                    }, void 0, false, {
                                        fileName: "[project]/components/InnovocationHistory.tsx",
                                        lineNumber: 148,
                                        columnNumber: 19
                                    }, this)
                                }, void 0, false, {
                                    fileName: "[project]/components/InnovocationHistory.tsx",
                                    lineNumber: 144,
                                    columnNumber: 15
                                }, this)
                            ]
                        }, void 0, true, {
                            fileName: "[project]/components/InnovocationHistory.tsx",
                            lineNumber: 121,
                            columnNumber: 13
                        }, this)
                    }, item.id, false, {
                        fileName: "[project]/components/InnovocationHistory.tsx",
                        lineNumber: 100,
                        columnNumber: 11
                    }, this))
            }, void 0, false, {
                fileName: "[project]/components/InnovocationHistory.tsx",
                lineNumber: 98,
                columnNumber: 7
            }, this)
        ]
    }, void 0, true, {
        fileName: "[project]/components/InnovocationHistory.tsx",
        lineNumber: 70,
        columnNumber: 5
    }, this);
}
}),
"[project]/lib/sorobantypes.ts [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "MOCK_CONTRACT_FUNCTIONS",
    ()=>MOCK_CONTRACT_FUNCTIONS,
    "generateMockResourceCost",
    ()=>generateMockResourceCost,
    "generateMockResult",
    ()=>generateMockResult
]);
const MOCK_CONTRACT_FUNCTIONS = [
    {
        name: 'transfer',
        inputs: [
            {
                name: 'from',
                type: 'address',
                description: 'Sender address'
            },
            {
                name: 'to',
                type: 'address',
                description: 'Recipient address'
            },
            {
                name: 'amount',
                type: 'u128',
                description: 'Amount to transfer'
            }
        ],
        outputs: 'bool'
    },
    {
        name: 'balance',
        inputs: [
            {
                name: 'account',
                type: 'address',
                description: 'Account address'
            }
        ],
        outputs: 'u128'
    },
    {
        name: 'mint',
        inputs: [
            {
                name: 'to',
                type: 'address',
                description: 'Recipient address'
            },
            {
                name: 'amount',
                type: 'u128',
                description: 'Amount to mint'
            }
        ],
        outputs: 'bool'
    },
    {
        name: 'symbol',
        inputs: [],
        outputs: 'string'
    },
    {
        name: 'decimals',
        inputs: [],
        outputs: 'u32'
    }
];
function generateMockResult(functionName, inputs) {
    const results = {
        transfer: {
            success: true,
            transaction_hash: '0x' + Math.random().toString(16).slice(2)
        },
        balance: Math.floor(Math.random() * 1000000),
        mint: {
            success: true,
            amount_minted: inputs.amount
        },
        symbol: 'USDC',
        decimals: 6
    };
    return results[functionName] || {
        success: true,
        message: 'Function executed'
    };
}
function generateMockResourceCost() {
    return {
        fee: '0.01',
        instructions: (Math.random() * 2 + 0.5).toFixed(1) + 'M'
    };
}
}),
"[project]/pages/index.tsx [ssr] (ecmascript)", ((__turbopack_context__) => {
"use strict";

__turbopack_context__.s([
    "default",
    ()=>Home
]);
var __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react/jsx-dev-runtime [external] (react/jsx-dev-runtime, cjs)");
// import Head from "next/head";
// import { ConnectButton } from "../components/ConnectButton";
// import { WalletModal } from "../components/WalletModal";
// export default function Home() {
//   return (
//     <>
//       <Head>
//         <title>SoroScope Dashboard</title>
//         <meta
//           name="description"
//           content="Soroban Resource Profiler – Web Dashboard"
//         />
//       </Head>
//       {/* Header */}
//       <header className="fixed top-0 left-0 right-0 z-30 bg-slate-950/80 backdrop-blur-sm border-b border-slate-800">
//         <div className="max-w-7xl mx-auto px-6 py-4 flex items-center justify-between">
//           <div className="flex items-center gap-3">
//             <h1 className="text-2xl font-bold text-white">SoroScope</h1>
//           </div>
//           {/* Wallet Connection in Top-Right */}
//           <div className="flex items-center gap-4">
//             <ConnectButton />
//           </div>
//         </div>
//       </header>
//       <main className="min-h-screen bg-slate-950 text-slate-100 pt-20">
//         <div className="max-w-7xl mx-auto px-6 py-8">
//           <div className="text-center mb-12">
//             <h1 className="text-4xl font-bold mb-4">SoroScope</h1>
//             <p className="text-slate-300">
//               Soroban Resource Profiler – Web Dashboard
//             </p>
//           </div>
//           <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
//             <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
//               <h3 className="text-xl font-semibold mb-3">Resource Profiling</h3>
//               <p className="text-slate-400">
//                 Analyze and profile Soroban smart contract resource usage
//               </p>
//             </div>
//             <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
//               <h3 className="text-xl font-semibold mb-3">
//                 Performance Metrics
//               </h3>
//               <p className="text-slate-400">
//                 Track CPU, memory, and storage consumption in real-time
//               </p>
//             </div>
//             <div className="bg-slate-900/50 border border-slate-800 rounded-xl p-6">
//               <h3 className="text-xl font-semibold mb-3">Optimize Contracts</h3>
//               <p className="text-slate-400">
//                 Identify bottlenecks and optimize your smart contracts
//               </p>
//             </div>
//           </div>
//         </div>
//       </main>
//       {/* Wallet Modal */}
//       <WalletModal />
//     </>
//   );
// }
// import { ConnectButton } from "../components/ConnectButton";
var __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__ = __turbopack_context__.i("[externals]/react [external] (react, cjs)");
var __TURBOPACK__imported__module__$5b$project$5d2f$components$2f$DynamicForm$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/components/DynamicForm.tsx [ssr] (ecmascript)");
var __TURBOPACK__imported__module__$5b$project$5d2f$components$2f$Resultviewer$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/components/Resultviewer.tsx [ssr] (ecmascript)");
var __TURBOPACK__imported__module__$5b$project$5d2f$components$2f$InnovocationHistory$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/components/InnovocationHistory.tsx [ssr] (ecmascript)");
var __TURBOPACK__imported__module__$5b$project$5d2f$lib$2f$sorobantypes$2e$ts__$5b$ssr$5d$__$28$ecmascript$29$__ = __turbopack_context__.i("[project]/lib/sorobantypes.ts [ssr] (ecmascript)");
;
;
;
;
;
;
function Home() {
    const [contractId, setContractId] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])('CAEZJVJ4N7P7GRUVD5NG5LYYH23AQHJUKQEUHW54LR5PGQX3V7FXD7Q');
    const [selectedFunction, setSelectedFunction] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(__TURBOPACK__imported__module__$5b$project$5d2f$lib$2f$sorobantypes$2e$ts__$5b$ssr$5d$__$28$ecmascript$29$__["MOCK_CONTRACT_FUNCTIONS"][0]);
    const [currentResult, setCurrentResult] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(null);
    const [loading, setLoading] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])(false);
    const [tab, setTab] = (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react__$5b$external$5d$__$28$react$2c$__cjs$29$__["useState"])('explorer');
    const { history, addToHistory } = (0, __TURBOPACK__imported__module__$5b$project$5d2f$components$2f$InnovocationHistory$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__["useInvocationHistory"])();
    const handleSimulate = async (inputs)=>{
        setLoading(true);
        try {
            // Simulate API delay
            await new Promise((resolve)=>setTimeout(resolve, 800));
            const result = {
                id: Math.random().toString(36).substring(7),
                functionName: selectedFunction.name,
                inputs,
                result: (0, __TURBOPACK__imported__module__$5b$project$5d2f$lib$2f$sorobantypes$2e$ts__$5b$ssr$5d$__$28$ecmascript$29$__["generateMockResult"])(selectedFunction.name, inputs),
                resourceCost: (0, __TURBOPACK__imported__module__$5b$project$5d2f$lib$2f$sorobantypes$2e$ts__$5b$ssr$5d$__$28$ecmascript$29$__["generateMockResourceCost"])(),
                timestamp: Date.now(),
                success: true
            };
            setCurrentResult(result);
            addToHistory(result);
        } catch (error) {
            const errorResult = {
                id: Math.random().toString(36).substring(7),
                functionName: selectedFunction.name,
                inputs,
                error: error instanceof Error ? error.message : 'Unknown error',
                timestamp: Date.now(),
                success: false
            };
            setCurrentResult(errorResult);
            addToHistory(errorResult);
        } finally{
            setLoading(false);
        }
    };
    return /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
        style: {
            minHeight: '100vh',
            backgroundColor: '#0f1117'
        },
        children: [
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("header", {
                style: {
                    backgroundColor: '#1a1f26',
                    borderBottom: '1px solid #30363d',
                    padding: '24px 0',
                    position: 'sticky',
                    top: 0,
                    zIndex: 100
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            maxWidth: '1200px',
                            margin: '0 auto',
                            padding: '0 24px'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h1", {
                                style: {
                                    margin: '0 0 12px 0',
                                    fontSize: '28px',
                                    fontWeight: '700',
                                    color: '#00d9ff',
                                    letterSpacing: '0.5px'
                                },
                                children: "Soroscope"
                            }, void 0, false, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 139,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                style: {
                                    margin: '0',
                                    color: '#8b949e',
                                    fontSize: '14px'
                                },
                                children: "Explore and test Soroban smart contracts with precision"
                            }, void 0, false, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 142,
                                columnNumber: 11
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/pages/index.tsx",
                        lineNumber: 138,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        className: "flex items-center gap-4",
                        children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(ConnectButton, {}, void 0, false, {
                            fileName: "[project]/pages/index.tsx",
                            lineNumber: 149,
                            columnNumber: 13
                        }, this)
                    }, void 0, false, {
                        fileName: "[project]/pages/index.tsx",
                        lineNumber: 148,
                        columnNumber: 11
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/pages/index.tsx",
                lineNumber: 128,
                columnNumber: 7
            }, this),
            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("main", {
                style: {
                    maxWidth: '1200px',
                    margin: '0 auto',
                    padding: '24px'
                },
                children: [
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            backgroundColor: '#161b22',
                            borderRadius: '8px',
                            padding: '24px',
                            marginBottom: '24px',
                            border: '1px solid #30363d'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("label", {
                                style: {
                                    display: 'block',
                                    marginBottom: '8px',
                                    fontWeight: '500',
                                    color: '#c9d1d9'
                                },
                                children: "Contract ID"
                            }, void 0, false, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 165,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("input", {
                                type: "text",
                                value: contractId,
                                onChange: (e)=>setContractId(e.target.value),
                                placeholder: "Enter Soroban contract ID",
                                style: {
                                    width: '100%',
                                    padding: '12px 16px',
                                    border: '1px solid #30363d',
                                    borderRadius: '6px',
                                    fontSize: '14px',
                                    fontFamily: 'monospace',
                                    boxSizing: 'border-box',
                                    backgroundColor: '#0d1117',
                                    color: '#c9d1d9'
                                }
                            }, void 0, false, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 168,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                style: {
                                    margin: '8px 0 0 0',
                                    fontSize: '12px',
                                    color: '#8b949e'
                                },
                                children: [
                                    "Contract ID: ",
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("code", {
                                        style: {
                                            color: '#00d9ff'
                                        },
                                        children: [
                                            contractId.substring(0, 20),
                                            "..."
                                        ]
                                    }, void 0, true, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 186,
                                        columnNumber: 26
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 185,
                                columnNumber: 11
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/pages/index.tsx",
                        lineNumber: 156,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            display: 'grid',
                            gridTemplateColumns: '1fr 1fr',
                            gap: '24px',
                            marginBottom: '24px'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                children: [
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                        style: {
                                            backgroundColor: '#161b22',
                                            borderRadius: '8px',
                                            padding: '24px',
                                            marginBottom: '24px',
                                            border: '1px solid #30363d'
                                        },
                                        children: [
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h2", {
                                                style: {
                                                    margin: '0 0 16px 0',
                                                    fontSize: '18px',
                                                    fontWeight: '600',
                                                    color: '#58a6ff'
                                                },
                                                children: "Available Functions"
                                            }, void 0, false, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 202,
                                                columnNumber: 15
                                            }, this),
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                                style: {
                                                    display: 'flex',
                                                    flexDirection: 'column',
                                                    gap: '8px'
                                                },
                                                children: __TURBOPACK__imported__module__$5b$project$5d2f$lib$2f$sorobantypes$2e$ts__$5b$ssr$5d$__$28$ecmascript$29$__["MOCK_CONTRACT_FUNCTIONS"].map((func)=>/*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                                                        onClick: ()=>{
                                                            setSelectedFunction(func);
                                                            setCurrentResult(null);
                                                        },
                                                        style: {
                                                            padding: '12px 16px',
                                                            backgroundColor: selectedFunction.name === func.name ? '#8957e5' : '#21262d',
                                                            color: selectedFunction.name === func.name ? '#fff' : '#c9d1d9',
                                                            border: selectedFunction.name === func.name ? '1px solid #8957e5' : '1px solid #30363d',
                                                            borderRadius: '6px',
                                                            textAlign: 'left',
                                                            cursor: 'pointer',
                                                            fontWeight: selectedFunction.name === func.name ? '600' : '500',
                                                            transition: 'all 0.2s',
                                                            fontSize: '14px'
                                                        },
                                                        onMouseEnter: (e)=>{
                                                            if (selectedFunction.name !== func.name) {
                                                                e.currentTarget.style.backgroundColor = '#21262d';
                                                                e.currentTarget.style.borderColor = '#8957e5';
                                                            }
                                                        },
                                                        onMouseLeave: (e)=>{
                                                            if (selectedFunction.name !== func.name) {
                                                                e.currentTarget.style.backgroundColor = '#21262d';
                                                                e.currentTarget.style.borderColor = '#30363d';
                                                            }
                                                        },
                                                        children: /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                                            style: {
                                                                display: 'flex',
                                                                justifyContent: 'space-between'
                                                            },
                                                            children: [
                                                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                                                    children: func.name
                                                                }, void 0, false, {
                                                                    fileName: "[project]/pages/index.tsx",
                                                                    lineNumber: 247,
                                                                    columnNumber: 23
                                                                }, this),
                                                                /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("span", {
                                                                    style: {
                                                                        fontSize: '12px',
                                                                        opacity: '0.7'
                                                                    },
                                                                    children: [
                                                                        func.inputs.length,
                                                                        " args"
                                                                    ]
                                                                }, void 0, true, {
                                                                    fileName: "[project]/pages/index.tsx",
                                                                    lineNumber: 248,
                                                                    columnNumber: 23
                                                                }, this)
                                                            ]
                                                        }, void 0, true, {
                                                            fileName: "[project]/pages/index.tsx",
                                                            lineNumber: 246,
                                                            columnNumber: 21
                                                        }, this)
                                                    }, func.name, false, {
                                                        fileName: "[project]/pages/index.tsx",
                                                        lineNumber: 214,
                                                        columnNumber: 19
                                                    }, this))
                                            }, void 0, false, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 212,
                                                columnNumber: 15
                                            }, this)
                                        ]
                                    }, void 0, true, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 193,
                                        columnNumber: 13
                                    }, this),
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                        style: {
                                            backgroundColor: '#161b22',
                                            borderRadius: '8px',
                                            padding: '24px',
                                            border: '1px solid #30363d'
                                        },
                                        children: [
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h2", {
                                                style: {
                                                    margin: '0 0 16px 0',
                                                    fontSize: '18px',
                                                    fontWeight: '600',
                                                    color: '#58a6ff'
                                                },
                                                children: selectedFunction.name
                                            }, void 0, false, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 266,
                                                columnNumber: 15
                                            }, this),
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(__TURBOPACK__imported__module__$5b$project$5d2f$components$2f$DynamicForm$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__["DynamicForm"], {
                                                func: selectedFunction,
                                                onSubmit: handleSimulate,
                                                loading: loading
                                            }, void 0, false, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 276,
                                                columnNumber: 15
                                            }, this)
                                        ]
                                    }, void 0, true, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 258,
                                        columnNumber: 13
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 192,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                children: [
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                        style: {
                                            display: 'flex',
                                            borderBottom: '1px solid #30363d',
                                            marginBottom: '24px',
                                            backgroundColor: '#161b22',
                                            borderRadius: '8px 8px 0 0',
                                            gap: '0'
                                        },
                                        children: [
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                                                onClick: ()=>setTab('explorer'),
                                                style: {
                                                    flex: 1,
                                                    padding: '12px 16px',
                                                    backgroundColor: 'transparent',
                                                    border: 'none',
                                                    borderBottom: tab === 'explorer' ? '2px solid #00d9ff' : 'none',
                                                    cursor: 'pointer',
                                                    fontSize: '14px',
                                                    fontWeight: tab === 'explorer' ? '600' : '500',
                                                    color: tab === 'explorer' ? '#00d9ff' : '#8b949e'
                                                },
                                                children: "Result"
                                            }, void 0, false, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 297,
                                                columnNumber: 15
                                            }, this),
                                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("button", {
                                                onClick: ()=>setTab('history'),
                                                style: {
                                                    flex: 1,
                                                    padding: '12px 16px',
                                                    backgroundColor: 'transparent',
                                                    border: 'none',
                                                    borderBottom: tab === 'history' ? '2px solid #00d9ff' : 'none',
                                                    cursor: 'pointer',
                                                    fontSize: '14px',
                                                    fontWeight: tab === 'history' ? '600' : '500',
                                                    color: tab === 'history' ? '#00d9ff' : '#8b949e'
                                                },
                                                children: [
                                                    "History (",
                                                    history.length,
                                                    ")"
                                                ]
                                            }, void 0, true, {
                                                fileName: "[project]/pages/index.tsx",
                                                lineNumber: 313,
                                                columnNumber: 15
                                            }, this)
                                        ]
                                    }, void 0, true, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 287,
                                        columnNumber: 13
                                    }, this),
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                        style: {
                                            backgroundColor: '#161b22',
                                            borderRadius: '0 8px 8px 8px',
                                            padding: '24px',
                                            border: '1px solid #30363d',
                                            borderTop: 'none'
                                        },
                                        children: tab === 'explorer' ? /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(__TURBOPACK__imported__module__$5b$project$5d2f$components$2f$Resultviewer$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__["ResultViewer"], {
                                            result: currentResult
                                        }, void 0, false, {
                                            fileName: "[project]/pages/index.tsx",
                                            lineNumber: 342,
                                            columnNumber: 17
                                        }, this) : /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])(__TURBOPACK__imported__module__$5b$project$5d2f$components$2f$InnovocationHistory$2e$tsx__$5b$ssr$5d$__$28$ecmascript$29$__["InvocationHistory"], {
                                            onSelectResult: (result)=>{
                                                setCurrentResult(result);
                                                setTab('explorer');
                                            }
                                        }, void 0, false, {
                                            fileName: "[project]/pages/index.tsx",
                                            lineNumber: 344,
                                            columnNumber: 17
                                        }, this)
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 332,
                                        columnNumber: 13
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 285,
                                columnNumber: 11
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/pages/index.tsx",
                        lineNumber: 190,
                        columnNumber: 9
                    }, this),
                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                        style: {
                            display: 'grid',
                            gridTemplateColumns: 'repeat(auto-fit, minmax(280px, 1fr))',
                            gap: '16px'
                        },
                        children: [
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                style: {
                                    backgroundColor: '#161b22',
                                    borderRadius: '8px',
                                    padding: '16px',
                                    borderLeft: '4px solid #00d9ff',
                                    border: '1px solid #30363d'
                                },
                                children: [
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h3", {
                                        style: {
                                            margin: '0 0 8px 0',
                                            fontSize: '14px',
                                            fontWeight: '600',
                                            color: '#00d9ff'
                                        },
                                        children: "Simulate"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 370,
                                        columnNumber: 13
                                    }, this),
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                        style: {
                                            margin: '0',
                                            fontSize: '13px',
                                            color: '#8b949e'
                                        },
                                        children: "Preview contract execution without signing or spending XLM"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 380,
                                        columnNumber: 13
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 361,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                style: {
                                    backgroundColor: '#161b22',
                                    borderRadius: '8px',
                                    padding: '16px',
                                    borderLeft: '4px solid #a371f7',
                                    border: '1px solid #30363d'
                                },
                                children: [
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h3", {
                                        style: {
                                            margin: '0 0 8px 0',
                                            fontSize: '14px',
                                            fontWeight: '600',
                                            color: '#a371f7'
                                        },
                                        children: "Invoke"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 394,
                                        columnNumber: 13
                                    }, this),
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                        style: {
                                            margin: '0',
                                            fontSize: '13px',
                                            color: '#8b949e'
                                        },
                                        children: "Execute real transactions via your connected wallet (Freighter/xBull)"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 404,
                                        columnNumber: 13
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 385,
                                columnNumber: 11
                            }, this),
                            /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("div", {
                                style: {
                                    backgroundColor: '#161b22',
                                    borderRadius: '8px',
                                    padding: '16px',
                                    borderLeft: '4px solid #fb8500',
                                    border: '1px solid #30363d'
                                },
                                children: [
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("h3", {
                                        style: {
                                            margin: '0 0 8px 0',
                                            fontSize: '14px',
                                            fontWeight: '600',
                                            color: '#fb8500'
                                        },
                                        children: "History"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 418,
                                        columnNumber: 13
                                    }, this),
                                    /*#__PURE__*/ (0, __TURBOPACK__imported__module__$5b$externals$5d2f$react$2f$jsx$2d$dev$2d$runtime__$5b$external$5d$__$28$react$2f$jsx$2d$dev$2d$runtime$2c$__cjs$29$__["jsxDEV"])("p", {
                                        style: {
                                            margin: '0',
                                            fontSize: '13px',
                                            color: '#8b949e'
                                        },
                                        children: "Track all function calls with full details and resource costs"
                                    }, void 0, false, {
                                        fileName: "[project]/pages/index.tsx",
                                        lineNumber: 428,
                                        columnNumber: 13
                                    }, this)
                                ]
                            }, void 0, true, {
                                fileName: "[project]/pages/index.tsx",
                                lineNumber: 409,
                                columnNumber: 11
                            }, this)
                        ]
                    }, void 0, true, {
                        fileName: "[project]/pages/index.tsx",
                        lineNumber: 354,
                        columnNumber: 9
                    }, this)
                ]
            }, void 0, true, {
                fileName: "[project]/pages/index.tsx",
                lineNumber: 154,
                columnNumber: 7
            }, this)
        ]
    }, void 0, true, {
        fileName: "[project]/pages/index.tsx",
        lineNumber: 126,
        columnNumber: 5
    }, this);
}
}),
"[externals]/next/dist/shared/lib/no-fallback-error.external.js [external] (next/dist/shared/lib/no-fallback-error.external.js, cjs)", ((__turbopack_context__, module, exports) => {

const mod = __turbopack_context__.x("next/dist/shared/lib/no-fallback-error.external.js", () => require("next/dist/shared/lib/no-fallback-error.external.js"));

module.exports = mod;
}),
];

//# sourceMappingURL=%5Broot-of-the-server%5D__d874b766._.js.map
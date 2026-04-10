import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const RATES: Record<string, number> = {
    "USD": 1.0,
    "EUR": 0.92,
    "GBP": 0.79,
    "CNY": 7.20,
    "JPY": 150.0
};

const SYMBOLS: Record<string, string> = {
    "USD": "$",
    "EUR": "€",
    "GBP": "£",
    "CNY": "¥",
    "JPY": "¥"
};

export function useCurrency() {
    const [currency, setCurrency] = useState("USD");
    const [customRate, setCustomRate] = useState<number | null>(null);

    const refresh = () => {
        invoke<string>("get_setting", { key: "currency" })
            .then(c => { if(c) setCurrency(c); })
            .catch(console.error);
        
        invoke<string>("get_setting", { key: "currency_rate" })
            .then(r => { if(r) setCustomRate(parseFloat(r)); })
            .catch(console.error);
    };

    useEffect(() => {
        refresh();
        const unlisten = listen("settings-changed", () => {
            refresh();
        });
        return () => {
            unlisten.then(f => f());
        };
    }, []);

    const rate = customRate || RATES[currency] || 1.0;
    const symbol = SYMBOLS[currency] || "$";

    const format = (amount: number) => {
        return `${symbol}${(amount * rate).toFixed(2)}`;
    };

    return { currency, format, currentRate: rate, currentSymbol: symbol };
}
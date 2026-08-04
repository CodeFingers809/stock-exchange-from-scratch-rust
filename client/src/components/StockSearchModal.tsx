"use client";

import React, { useState, useEffect, useRef } from "react";

export interface StockItem {
  symbol: string;
  name: string;
  exchanges: string[];
  ltp_ayushse: number;
  ltp_bohrase: number;
}

interface StockSearchModalProps {
  stocks: StockItem[];
  onSelectStock: (symbol: string, exchange: string) => void;
  onClose: () => void;
}

export function StockSearchModal({ stocks, onSelectStock, onClose }: StockSearchModalProps) {
  const [query, setQuery] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [onClose]);

  const filtered = stocks.filter(
    (s) =>
      s.symbol.toLowerCase().includes(query.toLowerCase()) ||
      s.name.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-start justify-center pt-[15vh]">
      <div className="bg-[#0f1420] border border-[#253049] w-full max-w-md shadow-2xl shadow-black/50">
        {/* Search */}
        <div className="flex items-center gap-2 px-3 py-2.5 border-b border-[#1e2740]">
          <span className="text-[#4a5568] text-xs">⌕</span>
          <input
            ref={inputRef}
            type="text"
            placeholder="Search ticker or name..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="flex-1 bg-transparent text-xs text-[#e2e8f0] placeholder-[#4a5568] focus:outline-none font-mono"
          />
          <span className="text-[9px] font-mono text-[#4a5568] border border-[#1e2740] px-1.5 py-0.5">
            ESC
          </span>
        </div>

        {/* Results */}
        <div className="max-h-[50vh] overflow-y-auto">
          {filtered.length === 0 ? (
            <div className="py-6 text-center text-[11px] text-[#4a5568] font-mono">No results</div>
          ) : (
            filtered.map((stock) => (
              <div key={stock.symbol} className="border-b border-[#1e2740] last:border-0">
                <div className="flex items-center justify-between px-3 py-2 hover:bg-[#151b2b] transition-colors">
                  <div>
                    <span className="font-mono text-xs font-semibold text-[#e2e8f0]">
                      {stock.symbol}
                    </span>
                    <span className="text-[10px] text-[#4a5568] ml-2">{stock.name}</span>
                  </div>
                  <div className="flex gap-1">
                    {stock.exchanges.map((ex) => (
                      <button
                        key={ex}
                        onClick={() => { onSelectStock(stock.symbol, ex); onClose(); }}
                        className="px-2 py-0.5 text-[9px] font-mono border border-[#1e2740] text-[#8494a7] hover:border-[#3b82f6] hover:text-[#3b82f6] hover:bg-[#3b82f608] transition-colors"
                      >
                        {ex}
                      </button>
                    ))}
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

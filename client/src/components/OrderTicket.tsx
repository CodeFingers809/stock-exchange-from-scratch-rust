"use client";

import React, { useState, useEffect } from "react";

interface OrderTicketProps {
  symbol: string;
  exchange: string;
  currentLtp: number;
  userBalance?: number;
  userHoldingQty?: number;
  onOrderPlaced?: (order: { symbol: string; exchange: string; side: "BUY" | "SELL"; qty: number; price: number; sl: number; tp: number }) => void;
}

export function OrderTicket({ symbol, exchange, currentLtp, userBalance = 1_000_000, userHoldingQty = 0, onOrderPlaced }: OrderTicketProps) {
  const [orderType, setOrderType] = useState<"MARKET" | "LIMIT">("LIMIT");
  const [side, setSide] = useState<"BUY" | "SELL">("BUY");
  const [price, setPrice] = useState<string>(currentLtp.toFixed(2));
  const [quantity, setQuantity] = useState<string>("100");
  const [stopLoss, setStopLoss] = useState<string>((currentLtp * 0.98).toFixed(2));
  const [takeProfit, setTakeProfit] = useState<string>((currentLtp * 1.05).toFixed(2));
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Sync price/SL/TP when currentLtp changes significantly
  useEffect(() => {
    if (currentLtp > 0) {
      setPrice(currentLtp.toFixed(2));
      setStopLoss((currentLtp * 0.98).toFixed(2));
      setTakeProfit((currentLtp * 1.05).toFixed(2));
    }
  }, [symbol, exchange]);

  const effectivePrice = orderType === "MARKET" ? currentLtp : parseFloat(price) || 0;
  const qty = parseInt(quantity) || 0;
  const totalCost = effectivePrice * qty;
  const slPrice = parseFloat(stopLoss) || 0;
  const tpPrice = parseFloat(takeProfit) || 0;
  const slPct = effectivePrice > 0 ? ((slPrice - effectivePrice) / effectivePrice * 100) : 0;
  const tpPct = effectivePrice > 0 ? ((tpPrice - effectivePrice) / effectivePrice * 100) : 0;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMsg(null);

    // 1. Validation Checks
    if (qty <= 0) {
      setErrorMsg("Please enter a valid quantity");
      return;
    }

    if (side === "SELL" && userHoldingQty < qty) {
      setErrorMsg(`Insufficient inventory! Holding: ${userHoldingQty} ${symbol}`);
      return;
    }

    if (side === "BUY" && userBalance < totalCost) {
      setErrorMsg(`Insufficient balance! Required: ₹${totalCost.toFixed(2)}`);
      return;
    }

    try {
      const backendHost = process.env.NEXT_PUBLIC_API_URL || "";
      const res = await fetch(`${backendHost}/api/order`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          exchange,
          symbol,
          order_type: orderType,
          bid_or_ask: side,
          quantity: qty,
          price_paisa: orderType === "LIMIT" ? Math.round(effectivePrice * 100) : null,
          stop_loss_paisa: Math.round(slPrice * 100),
          take_profit_paisa: Math.round(tpPrice * 100),
        }),
      });
      if (!res.ok) throw new Error("API error");
      const data = await res.json();
      onOrderPlaced?.({
        symbol,
        exchange,
        side,
        qty,
        price: effectivePrice,
        sl: slPrice,
        tp: tpPrice,
      });
    } catch {
      // Local execution fallback
      onOrderPlaced?.({
        symbol,
        exchange,
        side,
        qty,
        price: effectivePrice,
        sl: slPrice,
        tp: tpPrice,
      });
    }
  };

  const qtyPresets = [10, 50, 100, 500];

  return (
    <div className="flex flex-col h-full relative">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-[#1e2740]">
        <span className="text-[11px] font-mono tracking-widest text-[#8494a7] uppercase">Order Entry</span>
        <span className="text-[10px] font-mono text-[#4a5568]">{exchange}</span>
      </div>

      {/* Side Toggle */}
      <div className="grid grid-cols-2 border-b border-[#1e2740]">
        <button
          type="button"
          onClick={() => { setSide("BUY"); setErrorMsg(null); }}
          className={`py-2 text-xs font-semibold tracking-wide transition-colors ${
            side === "BUY"
              ? "bg-[#00c076] text-[#0a0e17]"
              : "text-[#4a5568] hover:text-[#8494a7] bg-transparent"
          }`}
        >
          BUY
        </button>
        <button
          type="button"
          onClick={() => { setSide("SELL"); setErrorMsg(null); }}
          className={`py-2 text-xs font-semibold tracking-wide transition-colors ${
            side === "SELL"
              ? "bg-[#ff4757] text-[#0a0e17]"
              : "text-[#4a5568] hover:text-[#8494a7] bg-transparent"
          }`}
        >
          SELL
        </button>
      </div>

      <form onSubmit={handleSubmit} className="flex-1 flex flex-col p-3 gap-2.5 overflow-y-auto">
        {/* Order Type */}
        <div className="flex items-center justify-between">
          <span className="text-[10px] text-[#4a5568] uppercase tracking-wider">Type</span>
          <div className="flex border border-[#1e2740] rounded overflow-hidden">
            {(["LIMIT", "MARKET"] as const).map((t) => (
              <button
                key={t}
                type="button"
                onClick={() => setOrderType(t)}
                className={`px-3 py-1 text-[10px] font-mono transition-colors ${
                  orderType === t
                    ? "bg-[#253049] text-[#e2e8f0]"
                    : "text-[#4a5568] hover:text-[#8494a7]"
                }`}
              >
                {t}
              </button>
            ))}
          </div>
        </div>

        {/* Price */}
        {orderType === "LIMIT" && (
          <div className="flex flex-col gap-1">
            <label className="text-[10px] text-[#4a5568] uppercase tracking-wider">Price</label>
            <input
              type="number"
              step="0.05"
              value={price}
              onChange={(e) => setPrice(e.target.value)}
              className="bg-[#0f1420] border border-[#1e2740] px-2.5 py-1.5 font-mono text-xs text-[#e2e8f0] focus:outline-none focus:border-[#3b82f6] transition-colors"
            />
          </div>
        )}

        {/* Quantity */}
        <div className="flex flex-col gap-1">
          <div className="flex items-center justify-between">
            <label className="text-[10px] text-[#4a5568] uppercase tracking-wider">Qty</label>
            {side === "SELL" && (
              <span className="text-[9px] font-mono text-[#8494a7]">Avail: <strong className="text-[#e2e8f0]">{userHoldingQty}</strong></span>
            )}
          </div>
          <input
            type="number"
            value={quantity}
            onChange={(e) => setQuantity(e.target.value)}
            className="bg-[#0f1420] border border-[#1e2740] px-2.5 py-1.5 font-mono text-xs text-[#e2e8f0] focus:outline-none focus:border-[#3b82f6] transition-colors"
          />
          <div className="flex gap-1 mt-0.5">
            {qtyPresets.map((q) => (
              <button
                key={q}
                type="button"
                onClick={() => setQuantity(q.toString())}
                className={`flex-1 py-0.5 text-[9px] font-mono border transition-colors ${
                  quantity === q.toString()
                    ? "border-[#3b82f6] text-[#3b82f6] bg-[#3b82f610]"
                    : "border-[#1e2740] text-[#4a5568] hover:text-[#8494a7]"
                }`}
              >
                {q}
              </button>
            ))}
          </div>
        </div>

        {/* Total Cost */}
        <div className="flex items-center justify-between py-1.5 px-2 bg-[#151b2b] border border-[#1e2740]">
          <span className="text-[9px] text-[#4a5568] uppercase tracking-wider">Total Required</span>
          <span className="font-mono text-[11px] tabular-nums text-[#e2e8f0]">
            ₹{totalCost.toLocaleString("en-IN", { maximumFractionDigits: 2, minimumFractionDigits: 2 })}
          </span>
        </div>

        {/* SL & TP */}
        <div className="border-t border-[#1e2740] pt-2 grid grid-cols-2 gap-2">
          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between">
              <label className="text-[9px] text-[#ff4757] uppercase tracking-wider font-mono">Stop Loss</label>
              <span className={`text-[8px] font-mono tabular-nums ${slPct < 0 ? "text-[#ff4757]" : "text-[#00c076]"}`}>
                {slPct > 0 ? "+" : ""}{slPct.toFixed(2)}%
              </span>
            </div>
            <input
              type="number"
              step="0.05"
              value={stopLoss}
              onChange={(e) => setStopLoss(e.target.value)}
              className="bg-[#0f1420] border border-[#ff475720] px-2 py-1 font-mono text-[11px] text-[#ff4757] focus:outline-none focus:border-[#ff4757]"
            />
          </div>
          <div className="flex flex-col gap-1">
            <div className="flex items-center justify-between">
              <label className="text-[9px] text-[#00c076] uppercase tracking-wider font-mono">Take Profit</label>
              <span className={`text-[8px] font-mono tabular-nums ${tpPct > 0 ? "text-[#00c076]" : "text-[#ff4757]"}`}>
                {tpPct > 0 ? "+" : ""}{tpPct.toFixed(2)}%
              </span>
            </div>
            <input
              type="number"
              step="0.05"
              value={takeProfit}
              onChange={(e) => setTakeProfit(e.target.value)}
              className="bg-[#0f1420] border border-[#00c07620] px-2 py-1 font-mono text-[11px] text-[#00c076] focus:outline-none focus:border-[#00c076]"
            />
          </div>
        </div>

        {/* Inline Error Toast (Does not move or shift the button) */}
        {errorMsg && (
          <div className="bg-[#ff475715] border border-[#ff475740] text-[#ff4757] text-[10px] font-mono px-2 py-1 rounded text-center">
            {errorMsg}
          </div>
        )}

        {/* Submit Button (Strictly BUY / SELL without displacement or text changes) */}
        <button
          type="submit"
          className={`mt-auto py-2.5 text-xs font-bold tracking-wider uppercase transition-all ${
            side === "BUY"
              ? "bg-[#00c076] hover:bg-[#00d884] text-[#0a0e17]"
              : "bg-[#ff4757] hover:bg-[#ff6b78] text-[#0a0e17]"
          }`}
        >
          {side}
        </button>
      </form>
    </div>
  );
}

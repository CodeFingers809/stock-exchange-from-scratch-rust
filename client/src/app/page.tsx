"use client";

import React, { useState, useEffect, useRef } from "react";
import { CandlestickChart } from "@/components/CandlestickChart";
import { OrderTicket } from "@/components/OrderTicket";
import { StockSearchModal, StockItem } from "@/components/StockSearchModal";

/* ────────────────────── TYPES ────────────────────── */

interface PanelState {
  id: string;
  symbol: string;
  exchange: string;
}

interface HftTelemetry {
  capital: number;
  realized_pnl: number;
  trades: number;
  wins: number;
  internal_lat_ns: number;
  internal_med_ns: number;
  rt_lat_ns: number;
  rt_med_ns: number;
  spread_paisa: number;
  inventory: number;
  ayushse_ltp: number;
  bohrase_ltp: number;
}

interface UserOrder {
  id: string;
  symbol: string;
  exchange: string;
  side: "BUY" | "SELL";
  qty: number;
  price: number;
  sl: number;
  tp: number;
  status: "OPEN" | "FILLED";
  time: number;
}

interface UserHolding {
  symbol: string;
  qty: number;
  avgPrice: number;
  currentLtp: number;
}

type RightTab = "ORDER" | "ACCOUNT" | "HFT";
type BottomTab = "ORDERS" | "HOLDINGS";

const INITIAL_STOCKS: StockItem[] = [
  { symbol: "TCS", name: "Tata Consultancy Services", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 3450.00, ltp_bohrase: 3448.50 },
  { symbol: "RELIANCE", name: "Reliance Industries Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 2890.50, ltp_bohrase: 2892.10 },
  { symbol: "INFY", name: "Infosys Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 1520.40, ltp_bohrase: 1518.90 },
  { symbol: "HDFCBANK", name: "HDFC Bank Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 1640.75, ltp_bohrase: 1642.00 },
  { symbol: "ICICIBANK", name: "ICICI Bank Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 1120.30, ltp_bohrase: 1121.50 },
  { symbol: "TATAMOTORS", name: "Tata Motors Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 980.60, ltp_bohrase: 979.80 },
  { symbol: "BHARTIARTL", name: "Bharti Airtel Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 1410.25, ltp_bohrase: 1412.00 },
  { symbol: "SBIN", name: "State Bank of India", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 825.40, ltp_bohrase: 824.90 },
  { symbol: "ITC", name: "ITC Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 465.80, ltp_bohrase: 466.15 },
  { symbol: "LTIM", name: "LTIMindtree Ltd", exchanges: ["AYUSHSE", "BOHRASE"], ltp_ayushse: 5120.00, ltp_bohrase: 5115.50 },
];

/* ────────────────────── HELPERS ────────────────────── */

function formatLatency(ns: number): string {
  if (ns === 0) return "—";
  if (ns < 1000) return `${ns}ns`;
  if (ns < 1_000_000) return `${(ns / 1000).toFixed(1)}µs`;
  return `${(ns / 1_000_000).toFixed(2)}ms`;
}

function formatCurrency(n: number): string {
  return n.toLocaleString("en-IN", { maximumFractionDigits: 2, minimumFractionDigits: 2 });
}

function formatCompact(n: number): string {
  const abs = Math.abs(n);
  if (abs >= 1_00_00_000) return `${(n / 1_00_00_000).toFixed(2)}Cr`;
  if (abs >= 1_00_000) return `${(n / 1_00_000).toFixed(2)}L`;
  if (abs >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toFixed(2);
}

type CandleMap = Record<string, any[]>;

function candleKey(symbol: string, exchange: string) {
  return `${symbol}:${exchange}`;
}

export default function TerminalPage() {
  const [stocks, setStocks] = useState<StockItem[]>(INITIAL_STOCKS);
  const [panels, setPanels] = useState<PanelState[]>([
    { id: "panel-1", symbol: "TCS", exchange: "AYUSHSE" },
  ]);
  const [activePanelId, setActivePanelId] = useState("panel-1");
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [candleMap, setCandleMap] = useState<CandleMap>({});
  const [wsConnected, setWsConnected] = useState(false);
  const [hft, setHft] = useState<HftTelemetry | null>(null);
  const [hftHistory, setHftHistory] = useState<{ t: number; capital: number; pnl: number }[]>([]);
  const [clock, setClock] = useState<Date | null>(null);
  const [rightTab, setRightTab] = useState<RightTab>("ORDER");
  const [bottomTab, setBottomTab] = useState<BottomTab>("ORDERS");
  const [showBottomPanel, setShowBottomPanel] = useState(true);
  const [userOrders, setUserOrders] = useState<UserOrder[]>([]);
  const [userHoldings, setUserHoldings] = useState<Record<string, UserHolding>>({});
  const [userBalance, setUserBalance] = useState(1_000_000); // User starts with ₹10L simulated balance
  const [marketLatNs, setMarketLatNs] = useState(450);
  const [marketRtLatNs, setMarketRtLatNs] = useState(1650);
  const [chartType, setChartType] = useState<"candle" | "line">("candle");
  const [orderbooks, setOrderbooks] = useState<Record<string, { bids: { price: number; qty: number; orders: number }[]; asks: { price: number; qty: number; orders: number }[] }>>({});
  const [isSimActive, setIsSimActive] = useState(false);
  const [isHftActive, setIsHftActive] = useState(true);
  const simTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  // Auto turn-off simulator on tab change or after 10 mins
  useEffect(() => {
    const handleVisibility = () => {
      if (document.hidden) {
        setIsSimActive(false);
      }
    };
    document.addEventListener("visibilitychange", handleVisibility);
    return () => document.removeEventListener("visibilitychange", handleVisibility);
  }, []);

  useEffect(() => {
    if (isSimActive) {
      if (simTimerRef.current) clearTimeout(simTimerRef.current);
      simTimerRef.current = setTimeout(() => {
        setIsSimActive(false);
      }, 10 * 60 * 1000); // 10 minutes auto-off
    } else {
      if (simTimerRef.current) clearTimeout(simTimerRef.current);
    }
  }, [isSimActive]);

  // Clock tick – initialised inside useEffect to avoid SSR/client hydration mismatch
  useEffect(() => {
    setClock(new Date()); // set immediately on mount
    const t = setInterval(() => setClock(new Date()), 1000);
    return () => clearInterval(t);
  }, []);

  // Keyboard shortcut
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "k") {
        e.preventDefault();
        setIsSearchOpen(true);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Fetch stocks list & global persistent candles from backend API on mount
  useEffect(() => {
      const backendHost = process.env.NEXT_PUBLIC_API_URL || "";
      fetch(`${backendHost}/api/stocks`)
        .then((res) => res.json())
        .then((data) => {
          if (Array.isArray(data) && data.length > 0) {
            setStocks(data);
          }
        })
        .catch(() => {});

      fetch(`${backendHost}/api/candles`)
        .then((res) => res.json())
        .then((data: any[]) => {
          if (Array.isArray(data) && data.length > 0) {
            const map: CandleMap = {};
            data.forEach((c) => {
              const key = candleKey(c.symbol, c.exchange);
              if (!map[key]) map[key] = [];
              map[key].push({
                time: c.time,
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
              });
            });
            setCandleMap(map);
          }
        })
        .catch(() => {});
    }, []);

  // WebSocket connection — updateCandle is defined inside here so it
  // never goes stale across renders (no dependency array issues).
  useEffect(() => {
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
    let mounted = true;

    const updateCandle = (sym: string, ex: string, price: number, timeSec: number) => {
      const key = `${sym}:${ex}`;
      const randVol = Math.floor(Math.random() * 120) + 10;
      setCandleMap((prev) => {
        const existing = prev[key] || [];
        if (!existing.length) {
          return { ...prev, [key]: [{ time: timeSec, open: price, high: price, low: price, close: price, volume: randVol }] };
        }
        const last = existing[existing.length - 1];
        if (last.time === timeSec) {
          const updated = { ...last, high: Math.max(last.high, price), low: Math.min(last.low, price), close: price, volume: last.volume + randVol };
          return { ...prev, [key]: [...existing.slice(0, -1), updated] };
        } else if (timeSec > last.time) {
          return { ...prev, [key]: [...existing, { time: timeSec, open: price, high: price, low: price, close: price, volume: randVol }] };
        }
        return prev;
      });
    };

    const connect = () => {
      if (!mounted) return;
      let wsUrl = process.env.NEXT_PUBLIC_WS_URL;
      if (!wsUrl) {
        // In local development or when pointing to local Rust backend:
        const isLocal = typeof window !== "undefined" && (window.location.hostname === "localhost" || window.location.hostname === "127.0.0.1");
        if (isLocal) {
          const wsProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
          wsUrl = `${wsProtocol}//localhost:3001/ws`;
        } else {
          // Deployed on Vercel: Vercel serverless does NOT support persistent WebSocket connections or running long-lived Rust backend processes on the same origin.
          // Fall back to local WS host so client can still connect to local Rust engine if running.
          wsUrl = "ws://localhost:3001/ws";
        }
      }
      const socket = new WebSocket(wsUrl);
      wsRef.current = socket;

      socket.onopen = () => {
        if (!mounted) { socket.close(); return; }
        setWsConnected(true);
      };

      socket.onmessage = (ev) => {
        if (!mounted) return;
        try {
          const msg = JSON.parse(ev.data);
          if (msg.type === "TICK") {
            const { symbol: sym, ayushse_ltp, bohrase_ltp, med_lat_ns, rt_med_lat_ns, ayushse_bids, ayushse_asks, bohrase_bids, bohrase_asks } = msg;

            if (med_lat_ns) setMarketLatNs(med_lat_ns);
            if (rt_med_lat_ns) setMarketRtLatNs(rt_med_lat_ns);

            if (ayushse_bids || ayushse_asks) {
              setOrderbooks((prev) => ({
                ...prev,
                [`AYUSHSE_${sym}`]: { bids: ayushse_bids || [], asks: ayushse_asks || [] },
                [`BOHRASE_${sym}`]: { bids: bohrase_bids || [], asks: bohrase_asks || [] },
              }));
            }

            setStocks((prev) =>
              prev.map((s) =>
                s.symbol === sym
                  ? { ...s, ltp_ayushse: ayushse_ltp || s.ltp_ayushse, ltp_bohrase: bohrase_ltp || s.ltp_bohrase }
                  : s
              )
            );

            const latestPrice = ayushse_ltp || bohrase_ltp;
            if (latestPrice > 0) {
              setUserHoldings((prev) => {
                if (!prev[sym]) return prev;
                return { ...prev, [sym]: { ...prev[sym], currentLtp: latestPrice } };
              });
            }

            const nowSec = Math.floor(Date.now() / 1000);
            const minSec = Math.floor(nowSec / 60) * 60;
            if (ayushse_ltp) updateCandle(sym, "AYUSHSE", ayushse_ltp, minSec);
            if (bohrase_ltp) updateCandle(sym, "BOHRASE", bohrase_ltp, minSec);

          } else if (msg.type === "HFT_TELEMETRY") {
            setHft(msg);
            setHftHistory((prev) => {
              const entry = { t: Date.now(), capital: msg.capital, pnl: msg.realized_pnl };
              const next = [...prev, entry];
              return next.length > 100 ? next.slice(-100) : next;
            });
          }
        } catch {}
      };

      socket.onclose = () => {
        if (!mounted) return;
        setWsConnected(false);
        wsRef.current = null;
        reconnectTimer = setTimeout(connect, 1500);
      };

      socket.onerror = () => {
        if (!mounted) return;
        setWsConnected(false);
      };
    };

    connect();

    return () => {
      mounted = false;
      if (reconnectTimer) clearTimeout(reconnectTimer);
      wsRef.current?.close();
      wsRef.current = null;
    };
  }, []);



  const activePanel = panels.find((p) => p.id === activePanelId) || panels[0];
  const activeStock = stocks.find((s) => s.symbol === activePanel?.symbol);
  const currentPrice = activePanel?.exchange === "AYUSHSE"
    ? activeStock?.ltp_ayushse || 0
    : activeStock?.ltp_bohrase || 0;
  const activeCandleData = candleMap[candleKey(activePanel?.symbol || "", activePanel?.exchange || "")] || [];
  const activeOrderbook = orderbooks[`${activePanel?.exchange}_${activePanel?.symbol}`] || { bids: [], asks: [] };
  const winRate = hft && hft.trades > 0 ? ((hft.wins / hft.trades) * 100).toFixed(1) : "—";

  const addPanel = (sym: string, ex: string) => {
    const existing = panels.find((p) => p.symbol === sym && p.exchange === ex);
    if (existing) {
      setActivePanelId(existing.id);
      return;
    }
    const id = `panel-${Date.now()}`;
    setPanels((p) => [...p, { id, symbol: sym, exchange: ex }]);
    setActivePanelId(id);
  };

  const removePanel = (id: string) => {
    if (panels.length <= 1) return;
    const next = panels.filter((p) => p.id !== id);
    setPanels(next);
    if (activePanelId === id) setActivePanelId(next[0].id);
  };

  const switchExchange = (ex: string) => {
    setPanels((prev) =>
      prev.map((p) => (p.id === activePanelId ? { ...p, exchange: ex } : p))
    );
  };

  // User Order Placement Logic (Updates balance and portfolio holdings)
  const handleOrderPlaced = (order: { symbol: string; exchange: string; side: "BUY" | "SELL"; qty: number; price: number; sl: number; tp: number }) => {
    const totalVal = order.price * order.qty;

    if (order.side === "BUY") {
      setUserBalance((prev) => prev - totalVal);
      setUserHoldings((prev) => {
        const existing = prev[order.symbol];
        if (existing) {
          const newQty = existing.qty + order.qty;
          const newAvg = (existing.avgPrice * existing.qty + totalVal) / newQty;
          return {
            ...prev,
            [order.symbol]: { ...existing, qty: newQty, avgPrice: newAvg, currentLtp: order.price },
          };
        }
        return {
          ...prev,
          [order.symbol]: { symbol: order.symbol, qty: order.qty, avgPrice: order.price, currentLtp: order.price },
        };
      });
    } else {
      setUserBalance((prev) => prev + totalVal);
      setUserHoldings((prev) => {
        const existing = prev[order.symbol];
        if (existing) {
          const newQty = existing.qty - order.qty;
          if (newQty <= 0) {
            const next = { ...prev };
            delete next[order.symbol];
            return next;
          }
          return {
            ...prev,
            [order.symbol]: { ...existing, qty: newQty, currentLtp: order.price },
          };
        }
        return prev;
      });
    }

    setUserOrders((prev) => [
      {
        id: crypto.randomUUID().slice(0, 8),
        symbol: order.symbol,
        exchange: order.exchange,
        side: order.side,
        qty: order.qty,
        price: order.price,
        sl: order.sl,
        tp: order.tp,
        status: "FILLED",
        time: Date.now(),
      },
      ...prev,
    ]);
  };

  const holdingsList = Object.values(userHoldings);
  const totalHoldingsValue = holdingsList.reduce((acc, h) => acc + h.qty * h.currentLtp, 0);

  return (
    <div className="flex flex-col h-screen w-screen overflow-hidden" style={{ background: "#0a0e17" }}>

      {/* ═══════════════ STATUS BAR (TOP) ═══════════════ */}
      <header className="h-7 flex items-center justify-between px-3 border-b border-[#1e2740] bg-[#0f1420] shrink-0 select-none">
        <div className="flex items-center gap-4 text-[10px] font-mono">
          <div className="flex items-center gap-1.5">
            <span className={`w-1.5 h-1.5 rounded-full ${wsConnected ? "bg-[#00c076] animate-pulse-dot" : "bg-[#ff4757]"}`} />
            <span className={wsConnected ? "text-[#00c076]" : "text-[#ff4757]"}>
              {wsConnected ? "LIVE" : "RECONNECTING"}
            </span>
          </div>
          <span className="text-[#253049]">│</span>
          <span className="text-[#4a5568]">CASH BAL:</span>
          <span className="text-[#e2e8f0] tabular-nums">₹{formatCurrency(userBalance)}</span>
          <span className="text-[#253049]">│</span>
          <span className="text-[#4a5568]">PORTFOLIO:</span>
          <span className="text-[#00c076] tabular-nums">₹{formatCurrency(userBalance + totalHoldingsValue)}</span>
          <span className="text-[#253049]">│</span>
          <span className="text-[#4a5568]">ORDERS:</span>
          <span className="text-[#e2e8f0] tabular-nums">{userOrders.length}</span>
        </div>

        <div className="flex items-center gap-4 text-[10px] font-mono">
          <span className="text-[#4a5568]">MED LAT:</span>
          <span className="text-[#8494a7] tabular-nums">{formatLatency(marketLatNs)}</span>
          <span className="text-[#253049]">│</span>
          <span className="text-[#4a5568]">FULL RT LAT:</span>
          <span className="text-[#f59e0b] tabular-nums">{formatLatency(marketRtLatNs)}</span>
          <span className="text-[#253049]">│</span>
          <span className="text-[#8494a7] tabular-nums">
            {clock ? clock.toLocaleTimeString("en-IN", { hour12: false }) + " IST" : ""}
          </span>
        </div>
      </header>

      {/* ═══════════════ MAIN WORKSPACE ═══════════════ */}
      <div className="flex flex-1 overflow-hidden">

        {/* ─── WATCHLIST (LEFT) ─── */}
        <aside className="w-48 border-r border-[#1e2740] bg-[#0f1420] flex flex-col shrink-0">
          <div className="flex items-center justify-between px-3 py-1.5 border-b border-[#1e2740]">
            <span className="text-[10px] font-mono tracking-widest text-[#4a5568] uppercase">Watchlist ({stocks.length})</span>
            <button
              onClick={() => setIsSearchOpen(true)}
              className="text-[#4a5568] hover:text-[#8494a7] transition-colors text-xs"
              title="Add (⌘K)"
            >+</button>
          </div>

          <div className="flex-1 overflow-y-auto">
            {stocks.map((s) => {
              const matchingPanel = panels.find((p) => p.symbol === s.symbol);
              const isActive = matchingPanel?.id === activePanelId;
              const displayLtp = s.ltp_ayushse || s.ltp_bohrase || 0;

              return (
                <div
                  key={s.symbol}
                  onClick={() => {
                    if (matchingPanel) {
                      setActivePanelId(matchingPanel.id);
                    } else {
                      addPanel(s.symbol, s.exchanges[0]);
                    }
                  }}
                  className={`flex items-center justify-between px-3 py-2 cursor-pointer border-l-2 transition-colors group ${
                    isActive
                      ? "border-l-[#3b82f6] bg-[#151b2b]"
                      : "border-l-transparent hover:bg-[#151b2b40]"
                  }`}
                >
                  <div>
                    <div className="flex items-center gap-1.5">
                      <span className="font-mono text-[11px] font-semibold text-[#e2e8f0]">{s.symbol}</span>
                      <span className="text-[7px] font-mono text-[#4a5568]">
                        BOTH
                      </span>
                    </div>
                    <span className={`font-mono text-[10px] tabular-nums ${displayLtp > 0 ? "text-[#8494a7]" : "text-[#4a5568]"}`}>
                      {displayLtp > 0 ? `₹${displayLtp.toFixed(2)}` : "Waiting..."}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>

          <div className="border-t border-[#1e2740] px-2 py-1.5 flex flex-col gap-1">
            <button
              onClick={() => setShowBottomPanel(!showBottomPanel)}
              className="text-[9px] font-mono text-[#4a5568] hover:text-[#8494a7] transition-colors text-left px-1"
            >
              {showBottomPanel ? "▾ Hide Orders & Holdings" : "▸ Show Orders & Holdings"}
            </button>
          </div>
        </aside>

        {/* ─── CENTER + BOTTOM ─── */}
        <div className="flex-1 flex flex-col overflow-hidden">

          {/* Chart Area */}
          <main className="flex-1 flex flex-col overflow-hidden">
            {/* Chart Toolbar */}
            <div className="h-8 flex items-center justify-between px-3 border-b border-[#1e2740] bg-[#0f1420] shrink-0">
              <div className="flex items-center gap-3">
                <span className="font-mono text-xs font-bold text-[#e2e8f0]">{activePanel?.symbol}</span>

                {/* Exchange Switch — always visible */}
                <div className="flex border border-[#1e2740] overflow-hidden">
                  {["AYUSHSE", "BOHRASE"].map((ex) => (
                    <button
                      key={ex}
                      onClick={() => switchExchange(ex)}
                      className={`px-2.5 py-0.5 text-[9px] font-mono transition-colors ${
                        activePanel?.exchange === ex
                          ? "bg-[#3b82f6] text-white"
                          : "text-[#4a5568] hover:text-[#8494a7] hover:bg-[#151b2b]"
                      }`}
                    >
                      {ex}
                    </button>
                  ))}
                </div>
              </div>

              <div className="flex items-center gap-3 font-mono text-[10px]">
                {/* Simulator On/Off Toggle */}
                <button
                  onClick={() => setIsSimActive(!isSimActive)}
                  className={`px-2 py-0.5 text-[9px] font-mono rounded border transition-colors flex items-center gap-1.5 ${
                    isSimActive
                      ? "bg-[#00c07620] border-[#00c076] text-[#00c076]"
                      : "bg-[#151b2b] border-[#1e2740] text-[#4a5568] hover:text-[#8494a7]"
                  }`}
                  title="Toggle Server Traffic Simulator (Auto-off in 10 mins or on tab close)"
                >
                  <span className={`w-1.5 h-1.5 rounded-full ${isSimActive ? "bg-[#00c076] animate-pulse" : "bg-[#4a5568]"}`} />
                  SIM: {isSimActive ? "ON" : "OFF"}
                </button>
                <span className="text-[#253049]">│</span>

                {/* Candle / Line Chart Toggle */}
                <div className="flex border border-[#1e2740] overflow-hidden">
                  <button
                    onClick={() => setChartType("candle")}
                    className={`px-2 py-0.5 text-[9px] transition-colors ${
                      chartType === "candle" ? "bg-[#253049] text-[#e2e8f0]" : "text-[#4a5568] hover:text-[#8494a7]"
                    }`}
                  >
                    Candle
                  </button>
                  <button
                    onClick={() => setChartType("line")}
                    className={`px-2 py-0.5 text-[9px] transition-colors ${
                      chartType === "line" ? "bg-[#253049] text-[#e2e8f0]" : "text-[#4a5568] hover:text-[#8494a7]"
                    }`}
                  >
                    Line
                  </button>
                </div>
                <span className="text-[#253049]">│</span>
                <span className="text-[#4a5568]">LTP</span>
                <span className={`font-semibold tabular-nums text-xs ${currentPrice > 0 ? "text-[#e2e8f0]" : "text-[#4a5568]"}`}>
                  {currentPrice > 0 ? `₹${currentPrice.toFixed(2)}` : "—"}
                </span>
              </div>
            </div>

            {/* Chart Area */}
            <div className="flex-1 relative overflow-hidden">
              {activeCandleData.length > 0 ? (
                <CandlestickChart
                  data={activeCandleData}
                  chartType={chartType}
                  symbol={activePanel?.symbol}
                  exchange={activePanel?.exchange}
                  onReset={() => {
                    setCandleMap({});
                    setStocks(INITIAL_STOCKS);
                    setUserOrders([]);
                    setUserHoldings({});
                    setUserBalance(1_000_000);
                    setHft(null);
                    setHftHistory([]);
                  }}
                />
              ) : (
                <div className="flex items-center justify-center h-full text-[#4a5568] font-mono text-xs">
                  Waiting for live market ticks for {activePanel?.symbol}...
                </div>
              )}
            </div>

            {/* 5v5 L2 Orderbook Depth Spread (just like TUI) */}
            <div className="h-28 border-t border-[#1e2740] bg-[#0b0f19] px-3 py-1.5 flex flex-col justify-between shrink-0 font-mono text-[9px]">
              <div className="flex items-center justify-between border-b border-[#1e2740] pb-1 text-[#4a5568] uppercase tracking-wider font-semibold">
                <span>📖 L2 Order Book Spread (5v5) — {activePanel?.exchange} ({activePanel?.symbol})</span>
                <span className="text-[#00c076]">LIVE FEED</span>
              </div>
              <div className="grid grid-cols-2 gap-4 flex-1 items-center pt-1">
                {/* BIDS (BUYERS) */}
                <div className="flex flex-col gap-0.5">
                  <div className="flex justify-between text-[#00c076] font-bold border-b border-[#1e2740]/50 pb-0.5">
                    <span>BUY (BIDS)</span>
                    <span>QTY</span>
                  </div>
                  {activeOrderbook.bids.slice(0, 5).map((bid, i) => (
                    <div key={i} className="flex justify-between text-[#8494a7] hover:bg-[#00c07610] px-1 rounded">
                      <span className="text-[#00c076] font-semibold">₹{bid.price.toFixed(2)}</span>
                      <span className="tabular-nums">{bid.qty} ({bid.orders} ord)</span>
                    </div>
                  ))}
                  {activeOrderbook.bids.length === 0 && (
                    <div className="text-[#4a5568] italic py-1">No resting bids</div>
                  )}
                </div>

                {/* ASKS (SELLERS) */}
                <div className="flex flex-col gap-0.5">
                  <div className="flex justify-between text-[#ff4757] font-bold border-b border-[#1e2740]/50 pb-0.5">
                    <span>SELL (ASKS)</span>
                    <span>QTY</span>
                  </div>
                  {activeOrderbook.asks.slice(0, 5).map((ask, i) => (
                    <div key={i} className="flex justify-between text-[#8494a7] hover:bg-[#ff475710] px-1 rounded">
                      <span className="text-[#ff4757] font-semibold">₹{ask.price.toFixed(2)}</span>
                      <span className="tabular-nums">{ask.qty} ({ask.orders} ord)</span>
                    </div>
                  ))}
                  {activeOrderbook.asks.length === 0 && (
                    <div className="text-[#4a5568] italic py-1">No resting asks</div>
                  )}
                </div>
              </div>
            </div>
          </main>

          {/* ─── BOTTOM PANEL (Orders / Holdings) ─── */}
          {showBottomPanel && (
            <div className="h-44 border-t border-[#1e2740] bg-[#0f1420] flex flex-col shrink-0">
              <div className="flex items-center gap-0 border-b border-[#1e2740]">
                {(["ORDERS", "HOLDINGS"] as BottomTab[]).map((tab) => (
                  <button
                    key={tab}
                    onClick={() => setBottomTab(tab)}
                    className={`px-4 py-1.5 text-[10px] font-mono tracking-wider transition-colors ${
                      bottomTab === tab
                        ? "text-[#e2e8f0] border-b-2 border-b-[#3b82f6]"
                        : "text-[#4a5568] hover:text-[#8494a7]"
                    }`}
                  >
                    {tab} ({tab === "ORDERS" ? userOrders.length : holdingsList.length})
                  </button>
                ))}
              </div>

              <div className="flex-1 overflow-y-auto p-2">
                {bottomTab === "ORDERS" && (
                  <div>
                    {userOrders.length === 0 ? (
                      <div className="text-center text-[10px] text-[#4a5568] font-mono py-4">No orders placed yet</div>
                    ) : (
                      <table className="w-full text-[10px] font-mono">
                        <thead>
                          <tr className="text-[#4a5568] border-b border-[#1e2740]">
                            <th className="text-left py-1 px-2">ID</th>
                            <th className="text-left py-1">Symbol</th>
                            <th className="text-left py-1">Exchange</th>
                            <th className="text-left py-1">Side</th>
                            <th className="text-right py-1">Qty</th>
                            <th className="text-right py-1">Price</th>
                            <th className="text-right py-1">SL</th>
                            <th className="text-right py-1">TP</th>
                            <th className="text-right py-1 px-2">Status</th>
                          </tr>
                        </thead>
                        <tbody>
                          {userOrders.map((o) => (
                            <tr key={o.id} className="border-b border-[#1e2740] hover:bg-[#151b2b] transition-colors">
                              <td className="py-1 px-2 text-[#8494a7]">{o.id}</td>
                              <td className="py-1 text-[#e2e8f0]">{o.symbol}</td>
                              <td className="py-1 text-[#4a5568]">{o.exchange}</td>
                              <td className={`py-1 ${o.side === "BUY" ? "text-[#00c076]" : "text-[#ff4757]"}`}>{o.side}</td>
                              <td className="py-1 text-right tabular-nums text-[#e2e8f0]">{o.qty}</td>
                              <td className="py-1 text-right tabular-nums text-[#8494a7]">₹{o.price.toFixed(2)}</td>
                              <td className="py-1 text-right tabular-nums text-[#ff4757]">₹{o.sl.toFixed(2)}</td>
                              <td className="py-1 text-right tabular-nums text-[#00c076]">₹{o.tp.toFixed(2)}</td>
                              <td className="py-1 px-2 text-right text-[#00c076]">{o.status}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </div>
                )}

                {bottomTab === "HOLDINGS" && (
                  <div>
                    {holdingsList.length === 0 ? (
                      <div className="text-center text-[10px] text-[#4a5568] font-mono py-4">
                        No holdings yet — place BUY orders to build portfolio
                      </div>
                    ) : (
                      <table className="w-full text-[10px] font-mono">
                        <thead>
                          <tr className="text-[#4a5568] border-b border-[#1e2740]">
                            <th className="text-left py-1 px-2">Symbol</th>
                            <th className="text-right py-1">Qty</th>
                            <th className="text-right py-1">Avg Price</th>
                            <th className="text-right py-1">LTP</th>
                            <th className="text-right py-1">Current Value</th>
                            <th className="text-right py-1 px-2">P&L</th>
                          </tr>
                        </thead>
                        <tbody>
                          {holdingsList.map((h) => {
                            const curVal = h.qty * h.currentLtp;
                            const invVal = h.qty * h.avgPrice;
                            const pnl = curVal - invVal;
                            const pnlPct = invVal > 0 ? (pnl / invVal) * 100 : 0;
                            return (
                              <tr key={h.symbol} className="border-b border-[#1e2740] hover:bg-[#151b2b] transition-colors">
                                <td className="py-1 px-2 text-[#e2e8f0] font-semibold">{h.symbol}</td>
                                <td className="py-1 text-right tabular-nums text-[#e2e8f0]">{h.qty}</td>
                                <td className="py-1 text-right tabular-nums text-[#8494a7]">₹{h.avgPrice.toFixed(2)}</td>
                                <td className="py-1 text-right tabular-nums text-[#e2e8f0]">₹{h.currentLtp.toFixed(2)}</td>
                                <td className="py-1 text-right tabular-nums text-[#e2e8f0]">₹{formatCurrency(curVal)}</td>
                                <td className={`py-1 px-2 text-right tabular-nums font-semibold ${pnl >= 0 ? "text-[#00c076]" : "text-[#ff4757]"}`}>
                                  {pnl >= 0 ? "+" : ""}₹{formatCurrency(pnl)} ({pnlPct.toFixed(2)}%)
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    )}
                  </div>
                )}
              </div>
            </div>
          )}
        </div>

        {/* ─── RIGHT PANEL (Tabs: Order / Account / HFT) ─── */}
        <aside className="w-64 border-l border-[#1e2740] bg-[#0f1420] shrink-0 flex flex-col">
          {/* Tab Header */}
          <div className="flex border-b border-[#1e2740]">
            {(["ORDER", "ACCOUNT", "HFT"] as RightTab[]).map((tab) => (
              <button
                key={tab}
                onClick={() => setRightTab(tab)}
                className={`flex-1 py-1.5 text-[9px] font-mono tracking-wider transition-colors ${
                  rightTab === tab
                    ? "text-[#e2e8f0] bg-[#151b2b] border-b-2 border-b-[#3b82f6]"
                    : "text-[#4a5568] hover:text-[#8494a7]"
                }`}
              >
                {tab}
              </button>
            ))}
          </div>

          {/* Tab Content */}
          <div className="flex-1 overflow-y-auto">
            {rightTab === "ORDER" && (
              <OrderTicket
                symbol={activePanel?.symbol || "TCS"}
                exchange={activePanel?.exchange || "AYUSHSE"}
                currentLtp={currentPrice}
                userBalance={userBalance}
                userHoldingQty={userHoldings[activePanel?.symbol || "TCS"]?.qty || 0}
                onOrderPlaced={handleOrderPlaced}
              />
            )}

            {rightTab === "ACCOUNT" && (
              <div className="p-3 flex flex-col gap-3">
                <div className="text-[10px] font-mono tracking-widest text-[#8494a7] uppercase">User Account Summary</div>

                <div className="bg-[#151b2b] border border-[#1e2740] p-2 flex flex-col gap-1.5">
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Cash Balance</span>
                    <span className="text-[#e2e8f0] tabular-nums font-semibold">₹{formatCurrency(userBalance)}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Holdings Value</span>
                    <span className="text-[#8494a7] tabular-nums">₹{formatCurrency(totalHoldingsValue)}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono border-t border-[#1e2740] pt-1">
                    <span className="text-[#4a5568]">Total Portfolio</span>
                    <span className="text-[#00c076] tabular-nums font-semibold">₹{formatCurrency(userBalance + totalHoldingsValue)}</span>
                  </div>
                </div>

                <div className="flex flex-col gap-1.5">
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Orders Executed</span>
                    <span className="text-[#e2e8f0] tabular-nums">{userOrders.length}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Holdings Count</span>
                    <span className="text-[#00c076] tabular-nums">{holdingsList.length}</span>
                  </div>
                </div>

                <div className="border-t border-[#1e2740] pt-2">
                  <div className="text-[9px] font-mono tracking-widest text-[#4a5568] uppercase mb-1.5">Active Holdings</div>
                  {holdingsList.length === 0 ? (
                    <div className="text-[10px] font-mono text-[#4a5568] text-center py-2">No holdings in portfolio</div>
                  ) : (
                    holdingsList.map((h) => {
                      const curVal = h.qty * h.currentLtp;
                      const invVal = h.qty * h.avgPrice;
                      const pnl = curVal - invVal;
                      return (
                        <div key={h.symbol} className="flex items-center justify-between py-1 border-b border-[#1e274015] text-[9px] font-mono">
                          <div>
                            <span className="text-[#e2e8f0] font-semibold">{h.symbol}</span>
                            <span className="text-[#4a5568] ml-1">({h.qty} qty)</span>
                          </div>
                          <span className={pnl >= 0 ? "text-[#00c076]" : "text-[#ff4757]"}>
                            {pnl >= 0 ? "+" : ""}₹{pnl.toFixed(0)}
                          </span>
                        </div>
                      );
                    })
                  )}
                </div>

                <div className="border-t border-[#1e2740] pt-2">
                  <div className="text-[9px] font-mono tracking-widest text-[#4a5568] uppercase mb-1.5">Recent Orders</div>
                  {userOrders.slice(0, 5).map((o) => (
                    <div key={o.id} className="flex items-center justify-between py-1 text-[9px] font-mono">
                      <span className={o.side === "BUY" ? "text-[#00c076]" : "text-[#ff4757]"}>
                        {o.side} {o.qty}×{o.symbol}
                      </span>
                      <span className="text-[#4a5568] tabular-nums">₹{o.price.toFixed(0)}</span>
                    </div>
                  ))}
                  {userOrders.length === 0 && (
                    <div className="text-[10px] font-mono text-[#4a5568] text-center py-2">—</div>
                  )}
                </div>
              </div>
            )}

            {rightTab === "HFT" && (
              <div className="p-3 flex flex-col gap-2.5">
                <div className="flex items-center justify-between">
                  <div className="text-[10px] font-mono tracking-widest text-[#3b82f6] uppercase">HFT Bot Telemetry</div>
                  <button
                    onClick={() => setIsHftActive(!isHftActive)}
                    className={`px-2 py-0.5 text-[9px] font-mono rounded border transition-colors flex items-center gap-1 ${
                      isHftActive
                        ? "bg-[#3b82f620] border-[#3b82f6] text-[#3b82f6]"
                        : "bg-[#151b2b] border-[#1e2740] text-[#4a5568] hover:text-[#8494a7]"
                    }`}
                  >
                    <span className={`w-1.5 h-1.5 rounded-full ${isHftActive ? "bg-[#3b82f6] animate-pulse" : "bg-[#4a5568]"}`} />
                    BOT: {isHftActive ? "ACTIVE" : "PAUSED"}
                  </button>
                </div>

                <div className="bg-[#151b2b] border border-[#1e2740] p-2 flex flex-col gap-1.5">
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">HFT Capital</span>
                    <span className="text-[#e2e8f0] tabular-nums">₹{formatCompact(hft?.capital || 0)}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Realized PnL</span>
                    <span className={`tabular-nums ${(hft?.realized_pnl || 0) >= 0 ? "text-[#00c076]" : "text-[#ff4757]"}`}>
                      {(hft?.realized_pnl || 0) >= 0 ? "+" : ""}₹{formatCompact(hft?.realized_pnl || 0)}
                    </span>
                  </div>
                </div>

                <div className="flex flex-col gap-1">
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Total Trades</span>
                    <span className="text-[#e2e8f0] tabular-nums">{(hft?.trades || 0).toLocaleString()}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Winning Trades</span>
                    <span className="text-[#00c076] tabular-nums">{(hft?.wins || 0).toLocaleString()}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Win Rate</span>
                    <span className="text-[#00c076] tabular-nums">{winRate}%</span>
                  </div>
                </div>

                <div className="border-t border-[#1e2740] pt-2 flex flex-col gap-1">
                  <div className="text-[9px] font-mono tracking-widest text-[#4a5568] uppercase">Inventory</div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Net Position</span>
                    <span className={`tabular-nums ${(hft?.inventory || 0) === 0 ? "text-[#8494a7]" : "text-[#f59e0b]"}`}>
                      {hft?.inventory || 0} shares
                    </span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Spread</span>
                    <span className={`tabular-nums ${(hft?.spread_paisa || 0) > 0 ? "text-[#00c076]" : "text-[#ff4757]"}`}>
                      ₹{((hft?.spread_paisa || 0) / 100).toFixed(2)}
                    </span>
                  </div>
                </div>

                <div className="border-t border-[#1e2740] pt-2 flex flex-col gap-1">
                  <div className="text-[9px] font-mono tracking-widest text-[#4a5568] uppercase">Latency</div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Internal Med</span>
                    <span className="text-[#8494a7] tabular-nums">{formatLatency(hft?.internal_med_ns || 0)}</span>
                  </div>
                  <div className="flex justify-between text-[10px] font-mono">
                    <span className="text-[#4a5568]">Round-Trip Med</span>
                    <span className="text-[#f59e0b] tabular-nums">{formatLatency(hft?.rt_med_ns || 0)}</span>
                  </div>
                </div>

                {hftHistory.length > 1 && (
                  <div className="border-t border-[#1e2740] pt-2">
                    <div className="text-[9px] font-mono tracking-widest text-[#4a5568] uppercase mb-1">Capital Growth</div>
                    <div className="flex items-end gap-[1px] h-8">
                      {hftHistory.slice(-40).map((h, i) => {
                        const min = Math.min(...hftHistory.slice(-40).map(x => x.capital));
                        const max = Math.max(...hftHistory.slice(-40).map(x => x.capital));
                        const range = max - min || 1;
                        const pct = ((h.capital - min) / range) * 100;
                        return (
                          <div
                            key={i}
                            className="flex-1 bg-[#00c07660]"
                            style={{ height: `${Math.max(pct, 5)}%` }}
                          />
                        );
                      })}
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </aside>
      </div>

      {/* ═══════════════ BOTTOM STATUS BAR ═══════════════ */}
      <footer className="h-5 flex items-center justify-between px-3 border-t border-[#1e2740] bg-[#0f1420] shrink-0 select-none">
        <div className="flex items-center gap-3 text-[9px] font-mono text-[#4a5568]">
          <a
            href="https://github.com/CodeFingers809"
            target="_blank"
            rel="noopener noreferrer"
            className="text-[#3b82f6] hover:underline flex items-center gap-1 font-semibold"
          >
            GitHub: @CodeFingers809
          </a>
          <span className="text-[#253049]">│</span>
          <span>AYUSHSE + BOHRASE</span>
        </div>
        <div className="flex items-center gap-3 text-[9px] font-mono text-[#4a5568]">
          <span>WS {wsConnected ? "OK" : "FAIL"}</span>
        </div>
      </footer>

      {isSearchOpen && (
        <StockSearchModal
          stocks={stocks}
          onSelectStock={(sym, ex) => addPanel(sym, ex)}
          onClose={() => setIsSearchOpen(false)}
        />
      )}
    </div>
  );
}

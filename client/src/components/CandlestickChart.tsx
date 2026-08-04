"use client";

import React, { useEffect, useRef, useState, useCallback } from "react";
import {
  createChart,
  ColorType,
  IChartApi,
  ISeriesApi,
  CandlestickSeries,
  LineSeries,
  HistogramSeries,
  CrosshairMode,
  IPriceLine,
  LineStyle,
  PriceLineOptions,
} from "lightweight-charts";
import { Trash2, Minus, TrendingUp, RefreshCw, Maximize2 } from "lucide-react";

interface CandlePoint {
  time: number;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

interface CandlestickChartProps {
  data: CandlePoint[];
  chartType?: "candle" | "line";
  symbol?: string;
  exchange?: string;
  onReset?: () => void;
}

type Timeframe = "1s" | "5s" | "15s" | "1m" | "5m" | "15m" | "1h";

const TIMEFRAMES: { label: string; value: Timeframe; secs: number }[] = [
  { label: "1s", value: "1s", secs: 1 },
  { label: "5s", value: "5s", secs: 5 },
  { label: "15s", value: "15s", secs: 15 },
  { label: "1m", value: "1m", secs: 60 },
  { label: "5m", value: "5m", secs: 300 },
  { label: "15m", value: "15m", secs: 900 },
  { label: "1h", value: "1h", secs: 3600 },
];

/** Aggregate raw 1-second candles into a larger timeframe */
function aggregateCandles(data: CandlePoint[], barSecs: number): CandlePoint[] {
  if (barSecs <= 1) return data;
  const buckets = new Map<number, CandlePoint>();
  for (const c of data) {
    const bucket = Math.floor(c.time / barSecs) * barSecs;
    const existing = buckets.get(bucket);
    if (!existing) {
      buckets.set(bucket, { ...c, time: bucket });
    } else {
      existing.high = Math.max(existing.high, c.high);
      existing.low = Math.min(existing.low, c.low);
      existing.close = c.close;
      existing.volume += c.volume;
    }
  }
  return Array.from(buckets.values()).sort((a, b) => a.time - b.time);
}

interface DrawnLine {
  id: string;
  price: number;
  color: string;
  style: "solid" | "dashed";
  label: string;
  priceLine: IPriceLine;
}

export function CandlestickChart({
  data,
  chartType = "candle",
  symbol,
  exchange,
  onReset,
}: CandlestickChartProps) {
  const chartContainerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<IChartApi | null>(null);
  const mainSeriesRef = useRef<ISeriesApi<"Candlestick" | "Line"> | null>(null);
  const volumeSeriesRef = useRef<ISeriesApi<"Histogram"> | null>(null);

  const [timeframe, setTimeframe] = useState<Timeframe>("1s");
  const [activeTool, setActiveTool] = useState<"none" | "horizontal" | "trend">("none");
  const [drawnLines, setDrawnLines] = useState<DrawnLine[]>([]);
  const [isResetting, setIsResetting] = useState(false);

  const tfSecs = TIMEFRAMES.find((t) => t.value === timeframe)?.secs ?? 1;
  const displayData = aggregateCandles(data, tfSecs);

  // ─── Initialize chart ────────────────────────────────────────────
  useEffect(() => {
    if (!chartContainerRef.current) return;
    const container = chartContainerRef.current;

    const chart = createChart(container, {
      layout: {
        background: { type: ColorType.Solid, color: "#0a0e17" },
        textColor: "#4a5568",
        fontFamily: "'JetBrains Mono', monospace",
        fontSize: 10,
      },
      grid: {
        vertLines: { color: "#1a213618" },
        horzLines: { color: "#1a213630" },
      },
      crosshair: {
        mode: CrosshairMode.Normal,
        vertLine: {
          color: "#3b82f650",
          width: 1,
          style: LineStyle.Dashed,
          labelBackgroundColor: "#1a2136",
        },
        horzLine: {
          color: "#3b82f650",
          width: 1,
          style: LineStyle.Dashed,
          labelBackgroundColor: "#1a2136",
        },
      },
      rightPriceScale: {
        borderColor: "#1e2740",
        scaleMargins: { top: 0.08, bottom: 0.22 },
        textColor: "#4a5568",
      },
      timeScale: {
        borderColor: "#1e2740",
        timeVisible: true,
        secondsVisible: true,
        rightOffset: 5,
        barSpacing: 8,
        fixLeftEdge: false,
      },
      handleScale: true,
      handleScroll: true,
      width: container.clientWidth,
      height: container.clientHeight,
    });

    chartRef.current = chart;

    // Main price series
    let mainSeries: ISeriesApi<"Candlestick" | "Line">;
    if (chartType === "candle") {
      mainSeries = chart.addSeries(CandlestickSeries, {
        upColor: "#00c076",
        downColor: "#ff4757",
        borderVisible: false,
        wickUpColor: "#00c07690",
        wickDownColor: "#ff475790",
      });
    } else {
      mainSeries = chart.addSeries(LineSeries, {
        color: "#3b82f6",
        lineWidth: 2,
      });
    }

    const volumeSeries = chart.addSeries(HistogramSeries, {
      priceFormat: { type: "volume" },
      priceScaleId: "",
    });
    volumeSeries.priceScale().applyOptions({
      scaleMargins: { top: 0.85, bottom: 0 },
    });

    mainSeriesRef.current = mainSeries;
    volumeSeriesRef.current = volumeSeries;

    // Resize observer
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        chart.applyOptions({ width, height });
      }
    });
    ro.observe(container);

    return () => {
      ro.disconnect();
      chart.remove();
      chartRef.current = null;
      mainSeriesRef.current = null;
      volumeSeriesRef.current = null;
    };
  }, [chartType]);

  // ─── Update data when displayData or timeframe changes ───────────
  useEffect(() => {
    if (!mainSeriesRef.current || !volumeSeriesRef.current || !displayData.length) return;

    const deduped = new Map<number, CandlePoint>();
    displayData.forEach((d) => deduped.set(d.time, d));
    const sorted = Array.from(deduped.values()).sort((a, b) => a.time - b.time);

    if (chartType === "candle") {
      (mainSeriesRef.current as ISeriesApi<"Candlestick">).setData(
        sorted.map((d) => ({ time: d.time as any, open: d.open, high: d.high, low: d.low, close: d.close }))
      );
    } else {
      (mainSeriesRef.current as ISeriesApi<"Line">).setData(
        sorted.map((d) => ({ time: d.time as any, value: d.close }))
      );
    }

    volumeSeriesRef.current.setData(
      sorted.map((d) => ({
        time: d.time as any,
        value: d.volume,
        color: d.close >= d.open ? "#00c07630" : "#ff475730",
      }))
    );
  }, [displayData, chartType]);

  // ─── Drawing: click to place price line ──────────────────────────
  const handleChartClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (activeTool === "none" || !chartRef.current || !mainSeriesRef.current) return;
      const container = chartContainerRef.current;
      if (!container) return;

      const rect = container.getBoundingClientRect();
      const y = e.clientY - rect.top;
      const price = mainSeriesRef.current.coordinateToPrice(y);
      if (price == null) return;

      const isHoriz = activeTool === "horizontal";
      const color = isHoriz ? "#3b82f6" : "#f59e0b";

      const opts: Partial<PriceLineOptions> = {
        price,
        color,
        lineWidth: 1,
        lineStyle: isHoriz ? LineStyle.Solid : LineStyle.Dashed,
        axisLabelVisible: true,
        title: isHoriz ? "S/R" : "Trend",
      };

      const priceLine = mainSeriesRef.current.createPriceLine(opts as PriceLineOptions);

      setDrawnLines((prev) => [
        ...prev,
        { id: crypto.randomUUID(), price, color, style: isHoriz ? "solid" : "dashed", label: isHoriz ? "S/R" : "Trend", priceLine },
      ]);
      setActiveTool("none");
    },
    [activeTool]
  );

  // ─── Clear all price lines ────────────────────────────────────────
  const clearLines = useCallback(() => {
    if (!mainSeriesRef.current) return;
    drawnLines.forEach((l) => {
      try { mainSeriesRef.current!.removePriceLine(l.priceLine); } catch (_) {}
    });
    setDrawnLines([]);
  }, [drawnLines]);

  // ─── Full reset: flush DB + Redis ────────────────────────────────
  const handleReset = useCallback(async () => {
    setIsResetting(true);
    try {
      await fetch("/api/reset", { method: "POST" });
      clearLines();
      onReset?.();
    } finally {
      setIsResetting(false);
    }
  }, [clearLines, onReset]);

  return (
    <div className="relative w-full h-full">

      {/* ── Top toolbar ── */}
      <div className="absolute top-2 left-2 z-10 flex items-center gap-1">

        {/* Timeframe selector */}
        <div className="flex border border-[#1e2740] overflow-hidden rounded">
          {TIMEFRAMES.map((tf) => (
            <button
              key={tf.value}
              onClick={() => setTimeframe(tf.value)}
              className={`px-1.5 py-0.5 text-[9px] font-mono transition-colors ${
                timeframe === tf.value
                  ? "bg-[#253049] text-[#e2e8f0]"
                  : "text-[#4a5568] hover:text-[#8494a7] hover:bg-[#151b2b]"
              }`}
            >
              {tf.label}
            </button>
          ))}
        </div>

        <div className="w-px h-4 bg-[#1e2740]" />

        {/* Drawing tools */}
        <button
          onClick={() => setActiveTool((t) => (t === "horizontal" ? "none" : "horizontal"))}
          title="Horizontal Price Level (S/R)"
          className={`p-1.5 rounded text-xs transition-colors border ${
            activeTool === "horizontal"
              ? "bg-[#3b82f6] border-[#3b82f6] text-white"
              : "border-[#1e2740] text-[#8494a7] hover:bg-[#151b2b]"
          }`}
        >
          <Minus className="w-3 h-3" />
        </button>
        <button
          onClick={() => setActiveTool((t) => (t === "trend" ? "none" : "trend"))}
          title="Trend / Alert Line"
          className={`p-1.5 rounded text-xs transition-colors border ${
            activeTool === "trend"
              ? "bg-[#f59e0b] border-[#f59e0b] text-white"
              : "border-[#1e2740] text-[#8494a7] hover:bg-[#151b2b]"
          }`}
        >
          <TrendingUp className="w-3 h-3" />
        </button>
        {drawnLines.length > 0 && (
          <button
            onClick={clearLines}
            title="Clear all drawings"
            className="p-1.5 rounded border border-[#1e2740] text-[#ff4757] hover:bg-[#ff475720] transition-colors"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        )}
      </div>

      {/* ── Top-right action buttons ── */}
      <div className="absolute top-2 right-2 z-10 flex items-center gap-1">
        {/* Autoscale / fit */}
        <button
          onClick={() => {
            if (!chartRef.current) return;
            chartRef.current.timeScale().fitContent();
            chartRef.current.priceScale("right").applyOptions({ autoScale: true });
          }}
          title="Fit all data to view"
          className="flex items-center gap-1 px-2 py-1 rounded border border-[#1e2740] text-[#8494a7] hover:bg-[#151b2b] hover:text-[#e2e8f0] text-[9px] font-mono transition-colors"
        >
          <Maximize2 className="w-2.5 h-2.5" />
          FIT
        </button>

        {/* Full data reset */}
        <button
          onClick={handleReset}
          disabled={isResetting}
          title="Full Reset: clear all candles, trades, portfolios & Redis"
          className="flex items-center gap-1 px-2 py-1 rounded border border-[#ff475740] text-[#ff4757] hover:bg-[#ff475715] text-[9px] font-mono transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-2.5 h-2.5 ${isResetting ? "animate-spin" : ""}`} />
          RESET
        </button>
      </div>

      {/* Active tool hint */}
      {activeTool !== "none" && (
        <div className="absolute top-10 left-2 z-10 px-2 py-1 bg-[#1e2740] text-[#e2e8f0] font-mono text-[9px] rounded shadow">
          Click chart to place {activeTool === "horizontal" ? "S/R level" : "trend line"}
        </div>
      )}

      {/* Drawn price lines legend */}
      {drawnLines.length > 0 && (
        <div className="absolute bottom-2 left-2 z-10 flex flex-col gap-0.5">
          {drawnLines.map((l) => (
            <div key={l.id} className="flex items-center gap-1 text-[8px] font-mono px-1.5 py-0.5 rounded bg-[#0f1420]/80 border border-[#1e2740]">
              <span style={{ color: l.color }}>─</span>
              <span className="text-[#8494a7]">{l.label}</span>
              <span className="text-[#e2e8f0] tabular-nums">₹{l.price.toFixed(2)}</span>
            </div>
          ))}
        </div>
      )}

      <div
        ref={chartContainerRef}
        onClick={handleChartClick}
        className={`w-full h-full ${activeTool !== "none" ? "cursor-crosshair" : "cursor-default"}`}
      />
    </div>
  );
}

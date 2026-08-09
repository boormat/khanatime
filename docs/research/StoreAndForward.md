# Store-and-Forward / Mesh Research

> Extracted from: PLAN.md (original planning document)
> See also: [Bluetooth.md](Bluetooth.md), [Berty.md](Berty.md)

---

## Current State (July 2026)

No production-ready, cross-platform, phone-only multihop mesh exists.
iOS is the bottleneck (background BLE restrictions, AWDL instead of WiFi Direct,
deprecated Multipeer Connectivity).

---

## Candidates

| Solution | Cross-Platform | Multihop | Store-and-Forward | Maturity | Notes |
|----------|---------------|----------|-------------------|----------|-------|
| @offline-protocol/mesh-sdk | iOS + Android | Yes | Yes | Early (v0.10) | React Native, Rust core, BLE + WiFi + Reticulum |
| Meshrabiya | Android only | Yes (true WiFi) | Yes | Early | 300Mbps, LGPL-3.0 |
| Bridgefy SDK | iOS + Android | Yes | Yes | Moderate | **Proprietary**, per-user pricing |
| Briar | Android only | Yes | Yes | High (maintenance) | GPLv3, P2P, no iOS |
| Berty | iOS + Android | Partial | Yes | Moderate | P2P via IPFS, BLE + WiFi, early beta |
| iOS WiFi Aware | Cross-platform potential | No (P2P only) | No | Brand new | iOS 26+, ~20 MB/s, future potential |

---

## P2P Communication Technology Matrix

| Technology | Android | iOS | Background iOS | Throughput |
|-----------|---------|-----|----------------|------------|
| Nearby Connections | Full | Limited (WiFi LAN) | Limited | 50-150 Mbps |
| WiFi Direct | Full | Not supported | No | 50-150 Mbps |
| WiFi Aware (NEW) | Supported | iOS 26+ | Yes (paired) | ~20 MB/s |
| Core Bluetooth | Full | Full (UUID filter) | Yes (throttled) | ~1 MB/s |
| BLE phone-to-phone | 50-140 KB/s | ~50 KB/s | — | ~0.5 Mbps |

---

## Cross-Platform P2P Today

- iOS-to-Android P2P is limited to BLE speeds (~0.5 Mbps)
- WiFi Aware cross-platform is the future path but not ready
- Matrix sync is the reliable fallback for connected devices

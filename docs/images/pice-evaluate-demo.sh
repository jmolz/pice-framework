#!/bin/bash
# Mock script that echoes exact pice evaluate output for GIF recording.
# This avoids needing ANTHROPIC_API_KEY / OPENAI_API_KEY for reproducible demos.

sleep 0.5

echo ""
echo "╔══════════════════════════════════════╗"
echo "║   Evaluation Report — Tier 2         ║"
echo "╠══════════════════════════════════════╣"
sleep 0.4
echo "║ ✅ Auth endpoints return 401     8/7 ║"
echo "║   All protected routes verified      ║"
sleep 0.3
echo "║ ✅ Password hashing uses bcrypt  9/7 ║"
echo "║   bcrypt with cost factor 12         ║"
sleep 0.3
echo "║ ✅ Session tokens expire in 24h  8/8 ║"
echo "║   24h expiry confirmed in tests      ║"
sleep 0.3
echo "║ ✅ No secrets in git history     7/7 ║"
echo "║   Clean scan across all commits      ║"
sleep 0.5
echo "╠══════════════════════════════════════╣"
echo "║  Adversarial Review                  ║"
echo "║  [consider] Rate limiting on logi... ║"
echo "║  [consider] Token rotation strate... ║"
sleep 0.4
echo "╠══════════════════════════════════════╣"
echo "║  Overall: PASS ✅                    ║"
echo "║  All contract criteria met           ║"
echo "╚══════════════════════════════════════╝"

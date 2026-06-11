#!/usr/bin/env bash
set -eo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

. "$ROOT/scripts/mac-setup.sh"

MODE="${1:-metal}"
FEATURES=""
if [ "$MODE" = "cpu" ]; then
  echo "Lunecent Voice - modo desenvolvimento (CPU)"
else
  echo "Lunecent Voice - modo desenvolvimento (Metal GPU)"
  FEATURES="--features metal"
fi

ensure_mac_deps

if pgrep -x "lunecent-voice" >/dev/null 2>&1; then
  echo "Encerrando instancia em execucao..."
  pkill -x "lunecent-voice" || true
  sleep 0.6
fi

if [ ! -d node_modules ]; then
  echo "Instalando dependencias do frontend (npm install)..."
  npm install
fi

echo "Iniciando Tauri em modo dev (compila e abre o app)..."
exec npx tauri dev $FEATURES

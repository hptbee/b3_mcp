package main

import "net/http"

func healthHandler(w http.ResponseWriter, r *http.Request) {
	w.WriteHeader(http.StatusOK)
}

func registerRoutes() {
	http.HandleFunc("/healthz", healthHandler)
}

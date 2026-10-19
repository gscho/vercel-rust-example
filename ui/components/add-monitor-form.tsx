"use client";

import { useState, type FormEvent } from "react";

import { api } from "@/lib/api";

export function AddMonitorForm({ onCreated }: { onCreated: () => void }) {
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string>();
  const [status, setStatus] = useState("");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const form = event.currentTarget;
    const data = new FormData(form);

    setSubmitting(true);
    setError(undefined);
    try {
      const monitor = await api.createMonitor({
        name: String(data.get("name")),
        url: String(data.get("url")),
        intervalSeconds: Number(data.get("intervalSeconds")),
      });
      form.reset();
      setStatus(`Added ${monitor.name}. The worker runs its first check within a few seconds.`);
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not add the endpoint.");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="vbg-custom-add-form" onSubmit={handleSubmit}>
      <div className="vbg-custom-add-form-fields">
        <div className="vbg-field">
          <label className="vbg-label" htmlFor="monitor-name">
            Name
          </label>
          <input id="monitor-name" name="name" required maxLength={100} autoComplete="off" />
        </div>
        <div className="vbg-field">
          <label className="vbg-label" htmlFor="monitor-url">
            URL
          </label>
          <input id="monitor-url" name="url" type="url" required autoComplete="off" />
        </div>
        <div className="vbg-field">
          <label className="vbg-label" htmlFor="monitor-interval">
            Check every
          </label>
          <div className="vbg-unit-field">
            <input
              id="monitor-interval"
              name="intervalSeconds"
              type="number"
              min={30}
              max={3600}
              step={1}
              defaultValue={60}
              required
              aria-describedby="monitor-interval-helper"
            />
            <span className="vbg-unit-suffix">seconds</span>
          </div>
          <p className="vbg-helper" id="monitor-interval-helper">
            From 30 to 3,600 seconds.
          </p>
        </div>
      </div>
      <div className="vbg-cluster">
        <button className="vbg-button" type="submit" disabled={submitting}>
          {submitting ? "Adding endpoint" : "Add endpoint"}
        </button>
        <p className="vbg-meta" role="status">
          {status}
        </p>
      </div>
      {error && (
        <p className="vbg-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

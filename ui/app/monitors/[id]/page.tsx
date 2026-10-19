import { Suspense } from "react";

import { MonitorDetail } from "@/components/monitor-detail";

export default function MonitorPage({ params }: PageProps<"/monitors/[id]">) {
  return (
    <Suspense>
      {params.then(({ id }) => (
        <MonitorDetail id={Number(id)} />
      ))}
    </Suspense>
  );
}

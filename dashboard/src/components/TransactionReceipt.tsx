"use client";

export type Receipt = {
  id: string;
  date: string;
  asset: string;
  amount: string | number;
  fee: string | number;
  status: string;
  hash: string;
};

export async function downloadReceiptPdf(r: Receipt): Promise<void> {
  const { jsPDF } = await import("jspdf");
  const doc = new jsPDF();
  const lines = [
    "Transaction Receipt",
    `ID: ${r.id}`,
    `Date: ${r.date}`,
    `Asset: ${r.asset}`,
    `Amount: ${r.amount}`,
    `Fee: ${r.fee}`,
    `Status: ${r.status}`,
    `Stellar hash: ${r.hash}`,
  ];
  lines.forEach((line, i) => doc.text(line, 10, 10 + i * 10));
  doc.save(`receipt-${r.id}.pdf`);
}

export default function TransactionReceipt({ receipt }: { receipt: Receipt }) {
  return (
    <div data-testid="transaction-receipt">
      <dl>
        <dt>ID</dt>
        <dd>{receipt.id}</dd>
        <dt>Date</dt>
        <dd>{receipt.date}</dd>
        <dt>Asset</dt>
        <dd>{receipt.asset}</dd>
        <dt>Amount</dt>
        <dd>{receipt.amount}</dd>
        <dt>Fee</dt>
        <dd>{receipt.fee}</dd>
        <dt>Status</dt>
        <dd>{receipt.status}</dd>
        <dt>Stellar hash</dt>
        <dd>{receipt.hash}</dd>
      </dl>
      <button onClick={() => downloadReceiptPdf(receipt)}>Download PDF</button>
    </div>
  );
}

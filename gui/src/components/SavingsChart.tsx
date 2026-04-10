import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from 'chart.js';
import { Line } from 'react-chartjs-2';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler
);

interface Props {
    data: [number, number][]; // [timestamp, amount]
    isDark: boolean;
}

export function SavingsChart({ data, isDark }: Props) {
    const options = {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
            legend: {
                display: false,
            },
            tooltip: {
                mode: 'index' as const,
                intersect: false,
                backgroundColor: isDark ? '#1e293b' : '#ffffff',
                titleColor: isDark ? '#f8fafc' : '#0f172a',
                bodyColor: isDark ? '#cbd5e1' : '#334155',
                borderColor: isDark ? '#334155' : '#e2e8f0',
                borderWidth: 1,
                padding: 10,
                callbacks: {
                    label: function(context: any) {
                        return `$${context.parsed.y.toFixed(2)} Saved`;
                    }
                }
            },
        },
        scales: {
            x: {
                display: false, // Hide dates for cleaner look, or show sparse
                grid: { display: false }
            },
            y: {
                display: true,
                grid: {
                    color: isDark ? '#334155' : '#f1f5f9',
                },
                ticks: {
                    color: isDark ? '#94a3b8' : '#64748b',
                    callback: (value: any) => '$' + value,
                },
                beginAtZero: true,
            },
        },
        interaction: {
            mode: 'nearest' as const,
            axis: 'x' as const,
            intersect: false
        },
    };

    const chartData = {
        labels: data.map(d => new Date(d[0] * 1000).toLocaleDateString()),
        datasets: [
            {
                fill: true,
                label: 'Cumulative Savings',
                data: data.map(d => d[1]),
                borderColor: 'rgb(79, 70, 229)', // Indigo 600
                backgroundColor: 'rgba(79, 70, 229, 0.1)',
                tension: 0.4, // Smooth curves
                pointRadius: 0,
                pointHoverRadius: 6,
            },
        ],
    };

    return (
        <div className="w-full h-full">
            <Line options={options} data={chartData} />
        </div>
    );
}

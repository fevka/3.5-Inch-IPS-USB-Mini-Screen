using LibreHardwareMonitor.Hardware;
using System;
using System.Globalization;
using System.Linq;
using System.Threading;

namespace LhmReader
{
    class Program
    {
        static void Main(string[] args)
        {
            var computer = new Computer
            {
                IsCpuEnabled = true,
                IsGpuEnabled = true,
                IsMemoryEnabled = true,
                IsMotherboardEnabled = true,
                IsControllerEnabled = true,
                IsNetworkEnabled = true,
                IsStorageEnabled = true,
                IsPsuEnabled = true
            };

            computer.Open();

            // Multiple update passes with delay - some sensors (Ryzen SMU)
            // need time to initialize or multiple reads to report valid values
            var visitor = new UpdateVisitor();
            for (int i = 0; i < 3; i++)
            {
                computer.Accept(visitor);
                Thread.Sleep(200);
            }

            foreach (var hardware in computer.Hardware)
            {
                foreach (var sensor in hardware.Sensors)
                {
                    if (sensor.Value.HasValue && !float.IsNaN(sensor.Value.Value) && !float.IsInfinity(sensor.Value.Value))
                    {
                        // Skip zero values for temperature sensors (not available)
                        if (sensor.SensorType == SensorType.Temperature && sensor.Value.Value == 0)
                            continue;

                        string hwType = hardware.HardwareType.ToString();
                        string key = $"{hwType}:{sensor.SensorType}:{sensor.Name}";
                        double val = sensor.Value.Value;

                        // Convert bytes to MB for SmallData sensors (GPU memory)
                        if (sensor.SensorType == SensorType.SmallData && val > 1_000_000)
                        {
                            val = val / 1024.0 / 1024.0;
                        }

                        Console.WriteLine(string.Format(CultureInfo.InvariantCulture, "{0}|{1:F1}", key, val));
                    }
                }

                // Recurse into sub-hardware
                foreach (var sub in hardware.SubHardware)
                {
                    foreach (var sensor in sub.Sensors)
                    {
                        if (sensor.Value.HasValue && !float.IsNaN(sensor.Value.Value) && !float.IsInfinity(sensor.Value.Value))
                        {
                            if (sensor.SensorType == SensorType.Temperature && sensor.Value.Value == 0)
                                continue;

                            string hwType = sub.HardwareType.ToString();
                            string key = $"{hwType}:{sensor.SensorType}:{sensor.Name}";
                            double val = sensor.Value.Value;

                            if (sensor.SensorType == SensorType.SmallData && val > 1_000_000)
                            {
                                val = val / 1024.0 / 1024.0;
                            }

                            Console.WriteLine(string.Format(CultureInfo.InvariantCulture, "{0}|{1:F1}", key, val));
                        }
                    }
                }
            }

            computer.Close();
        }
    }

    public class UpdateVisitor : IVisitor
    {
        public void VisitComputer(IComputer computer)
        {
            computer.Traverse(this);
        }

        public void VisitHardware(IHardware hardware)
        {
            hardware.Update();
            foreach (var sub in hardware.SubHardware)
                sub.Accept(this);
        }

        public void VisitSensor(ISensor sensor) { }
        public void VisitParameter(IParameter parameter) { }
    }
}

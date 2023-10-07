import 'package:get_10101/hybrid_logger.dart';
import 'package:get_10101/simple_utc_printer.dart';
import 'package:logger/logger.dart';

Logger get logger => AppLogger.instance;

class AppLogger {
  static final Logger _logger = Logger(
      output: HybridOutput(),
      printer: SimpleUTCPrinter(
          // Colorful log messages
          colors: false,
          // Should each log print contain a timestamp
          printTime: true));

  // Getter to access the logger instance
  static final instance = _logger;
}

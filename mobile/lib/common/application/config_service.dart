import 'package:f_logs/model/flog/flog.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/util/preferences.dart';

class ConfigService {
  rust.Config _config = Environment.parse();

  String commit = const String.fromEnvironment('COMMIT', defaultValue: 'not available');
  String branch = const String.fromEnvironment('BRANCH', defaultValue: 'not available');

  String buildNumber = "";
  String version = "";

  bool devMode = (const String.fromEnvironment('DEV_MODE', defaultValue: 'false')) == 'true';

  ConfigService() {
    determineConfig(_config).then((config) {
      _config = config;
    });
  }

  rust.Config getConfig() {
    return _config;
  }

  Future<rust.Config> determineConfig(rust.Config parsedConfig) async {
    final network = await Preferences.instance.getNetwork();
    switch (network) {
      case Network.regtest:
        if (devMode) {
          FLog.info(
              text:
                  "Dev mode enabled: using provided `--dart-define` variables instead of public regtest node config");
          return parsedConfig;
        } else {
          return rust.api.regtestConfig();
        }
      case Network.mainnet:
        return rust.api.mainnetConfig();
    }
  }
}

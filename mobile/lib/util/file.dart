import 'dart:io';

import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/environment.dart';
import 'package:path_provider/path_provider.dart';

Future<String> getSeedFilePath() async {
  final config = Environment.parse();
  final seedDir = (await getActualSeedPath(config)).path;

  final seedFilePath = '$seedDir/seed';
  return seedFilePath;
}

Future<bool> isSeedFilePresent() async {
  final seedFilePath = await getSeedFilePath();
  logger.d("Scanning for seed file in: $seedFilePath");
  return File(seedFilePath).existsSync();
}

/// Take into account that the backend creates seed dir inside "network"
/// sub-directory
Future<Directory> getActualSeedPath(bridge.Config config) async {
  final seedDir = (await getApplicationSupportDirectory()).path;

  // We need to adjust the naming of the seed dir for mainnet
  final network = config.network == "mainnet" ? "bitcoin" : config.network;
  final seedPath = '$seedDir/$network';
  return Directory(seedPath);
}

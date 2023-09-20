import 'package:version/version.dart';

class CoordinatorVersion {
  final Version version;

  CoordinatorVersion(this.version);

  CoordinatorVersion.fromJson(Map<String, dynamic> json) : version = Version.parse(json['version']);
}

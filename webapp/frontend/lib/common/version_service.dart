import 'package:flutter/material.dart';
import 'dart:convert';
import 'package:get_10101/common/http_client.dart';

class Version {
  final String version;
  final String commitHash;
  final String branch;

  const Version({required this.version, required this.commitHash, required this.branch});

  factory Version.fromJson(Map<String, dynamic> json) {
    return switch (json) {
      {
        'version': String version,
        'commit_hash': String commitHash,
        'branch': String branch,
      } =>
        Version(version: version, commitHash: commitHash, branch: branch),
      _ => throw const FormatException('Failed to load version.'),
    };
  }
}

class VersionService {
  const VersionService();

  Future<Version> fetchVersion() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/version'));

    if (response.statusCode == 200) {
      var jsonResponse = jsonDecode(response.body);
      if (jsonResponse == null) {
        throw FlutterError("Failed to fetch version");
      }
      return Version.fromJson(jsonResponse as Map<String, dynamic>);
    } else {
      throw FlutterError("Failed to fetch version");
    }
  }
}

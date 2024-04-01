import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/logger/logger.dart';

class AuthService {
  Future<bool> isLoggedIn() async {
    try {
      final response = await HttpClientManager.instance.get(Uri(path: '/api/node'));
      return response.statusCode == 200;
    } catch (error) {
      return false;
    }
  }

  Future<void> signIn(String password) async {
    final response = await HttpClientManager.instance.post(Uri(path: '/api/login'),
        headers: <String, String>{
          'Content-Type': 'application/json; charset=UTF-8',
        },
        body: jsonEncode(<String, dynamic>{'password': password}));

    if (response.statusCode != 200) {
      throw FlutterError("Failed to login");
    }

    logger.i("Successfully logged in!");
  }

  Future<void> signOut() async {
    await HttpClientManager.instance.get(Uri(path: '/api/logout'));
  }
}

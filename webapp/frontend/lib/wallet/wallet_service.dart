import 'package:flutter/material.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/balance.dart';
import 'package:http/http.dart' as http;

class WalletService {
  const WalletService();

  Future<Balance> getBalance() async {
    // todo: fetch balance from backend
    return Balance(Amount(123454), Amount(124145214));
  }

  Future<String> getNewAddress() async {
    // TODO(holzeis): this should come from the config
    const port = "3001";
    const host = "localhost";

    try {
      final response = await http.get(Uri.http('$host:$port', '/api/newaddress'));

      if (response.statusCode == 200) {
        return response.body;
      } else {
        throw FlutterError("Failed to fetch new address");
      }
    } catch (e) {
      throw FlutterError("Failed to fetch new address. $e");
    }
  }

  Future<void> sendPayment(String address, Amount amount, Amount fee) async {
    // todo: send payment
    throw UnimplementedError("todo");
  }
}
